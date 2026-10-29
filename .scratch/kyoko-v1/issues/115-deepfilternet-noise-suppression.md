# 115 — Noise suppression close to Krisp: DeepFilterNet3, on by default

**What to build:** Follow-up from 100. The user wants noise suppression "the best, close to Krisp as possible", on by default (2026-10-29). Today `crates/voice` runs RNNoise through `nnnoiseless` 0.5 (the old 22-band model), off by default. Replace it with DeepFilterNet3 (MIT/Apache, full-band 48 kHz, 10 ms hop matching kyoko's frame) as the default, and offer RNNoise 0.2 (BSD, the 2024 model) as a low-CPU choice. The setting becomes a choice like Discord's: **High quality** (DeepFilterNet3, default), **Low CPU** (RNNoise 0.2), **Off**. `nnnoiseless` leaves the tree.

**Blocked by:** none (touches `crates/voice` capture chain; 104 camera send and 113 stream audio also touch `crates/voice`: rebase on whichever lands first)

**Status:** resolved

- [x] DeepFilterNet3 runs in the capture chain after resampling to 48 kHz and before input volume and the gate; standard model (2-frame lookahead), inference on its own thread or inline if it stays well under the 10 ms budget; measured ms per frame and added latency written in `docs/voice.md`
- [x] Integration: `deep_filter` is git-only at 0.5.x with tract 0.21 and no release since 2024: vendor or pin it (licence notices kept, `cargo deny` green, no GPL), model weights embedded or shipped beside the exe with a hash check; release exe growth measured and stated
- [x] RNNoise 0.2 (xiph/rnnoise, built with `cc` under MSVC, weights fetched by hash at build time or vendored) as Low CPU; `nnnoiseless` removed
- [x] Setting `voice.noise_suppression` becomes `high_quality` / `low_cpu` / `off`, default `high_quality` (no migration: nobody uses kyoko yet); the Voice page shows the three choices; switching applies live without reconnecting
- [x] Session tests (`AudioIo::Manual`): public-domain or generated speech mixed with noise at 0, 5 and 10 dB through the pipeline; after delay alignment, SI-SDR improves by a set margin, noise-only stretches drop by at least 20 dB, speech level stays within 3 dB; the same for Low CPU with its own margins; Off passes audio through
- [x] If CPU per frame is too high on this machine (say over 3 ms), stop and report numbers instead of shipping it as default
- [x] `THIRD_PARTY_NOTICES.md` gains DeepFilterNet, tract, RNNoise (and the missing libopus entry from 97); `docs/voice.md` updated; settings capture checked by eye

## Research (2026-10-29, orchestrator's research agent; clones in `nguyenvu/.research/noise/`, untrusted, read only)

- Krisp (Discord's) removes non-voice background noise on your side only; it is proprietary. Removing other people's voices (Krisp's separate voice cancellation) has no permissive open model without an enrollment sample: out of scope.
- DeepFilterNet3: PESQ 3.17 on VoiceBank-DEMAND, RTF 0.19 on one i5-8250U thread via Rust/tract, 960 FFT / 480 hop, 2 lookahead frames (~30 ms algorithmic, paper says 40 ms overall). https://arxiv.org/html/2305.08227, https://github.com/Rikorose/DeepFilterNet. Model ~8.6 MB fp32 ONNX. Low-latency variant has 0 lookahead but is 39 MB. Estimate 1-2 ms per frame, +3-6 MB exe for tract.
- RNNoise 0.2 (April 2024): new 32-band model, AVX2 paths, public training data, a "little" model. https://github.com/xiph/rnnoise. Estimate well under 0.5 ms per frame, +10 ms latency.
- Rejected: Meta Denoiser (non-commercial), GTCRN and FRCRN (16 kHz only), PercepNet (no weights), NSNet2 (weights licence unclear), DPDFNet (weights licence not stated; revisit later).

## Answer

Merged `3d0b285` (lane C, worker `315fd3e1`), verified by the orchestrator twice (at `7cf70a5`, then the final commit): fmt, clippy, build, test --workspace, xtask test, headless-run, deny, luau0-src 0.22.0+luau740 green; tls-capture against chrome-156 0/0; kyoko-only captures `settings-voice`, `settings-voice-processing` (new) checked by eye.

- New headless crate `kyoko-denoise`. High Quality: DeepFilterNet3, libDF's inference path ported onto tract 0.23.9, model shipped as NNEF (`crates/denoise/models/DeepFilterNet3.nnef.tar.gz`, SHA-checked at load). Low CPU: RNNoise xiph head `70f1d25` (0.2 line, 2025-01 model) built with `cc`, weights as a generated SHA-checked blob (`rnnoise.bin`). Off passes through. `nnnoiseless` removed.
- Setting `voice.noise_suppression` = `high_quality` (default) / `low_cpu` / `off`, applied live without reconnecting; an old bool falls back to the default. Runs right after resampling, before input volume, meter and gate, in calls and the mic test. RNNoise covers the ~0.2-0.3 s DFN3 load or a load failure. Added latency 30 ms (HQ), 10 ms (Low CPU). CPU per 10 ms hop: HQ ~1.3 ms mean, Low CPU ~0.2 ms.
- Size (user's decision: shrink before merging): release exe 89.0 MB -> 117.4 MB (+28.4; was +36.4 with the ONNX path). The NNEF output is bit-identical to the ONNX path (max sample difference 0 on all bench clips). tract-onnx, tract-pulse, tract-transformers only under the optional `onnx` feature; regenerate the NNEF with `cargo run --release -p kyoko-denoise --features onnx,testing --example nnef_export -- <DeepFilterNet3_onnx.tar.gz> models/DeepFilterNet3.nnef.tar.gz` (checks the upstream SHA `c94d91f7…`, reruns the comparison; a rebuild changes the file hash, so update `DEEP_FILTER_SHA256`). Going smaller means replacing tract.
- Speech fixture (user-approved swap from Windows TTS): `crates/denoise/fixtures/speech.wav`, 4.2 s of the LibriVox "Gettysburg Address" read by John Greenman (public domain), provenance in `crates/denoise/fixtures/README.md` and THIRD_PARTY_NOTICES. Noise is generated in code.
- Session tests (`AudioIo::Manual`) compare each mode against Off through the call (Opus caps SI-SDR near 7 dB): HQ +5.2 / +3.2 / +1.1 dB at 0 / 5 / 10 dB SNR, Low CPU +3.7 / +1.8 / -0.1; noise-only stretches drop > 40 dB; speech level within 1.4 dB. Voice harnesses start with suppression Off so tone tests keep testing the gate.
- For 116: after a loud pure tone DFN3 lets noise through for > 1.5 s (normalisation drift), so warm up tests on noise, not tones; `denoise::testing` (speech, noise, `Clip`, SI-SDR, alignment) is reusable. RNNoise's `rnnoise_model_free` crashes on buffer-loaded models: one model per process. Stream audio (118) should bypass mic noise suppression.
