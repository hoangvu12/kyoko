# 115 — Noise suppression close to Krisp: DeepFilterNet3, on by default

**What to build:** Follow-up from 100. The user wants noise suppression "the best, close to Krisp as possible", on by default (2026-10-29). Today `crates/voice` runs RNNoise through `nnnoiseless` 0.5 (the old 22-band model), off by default. Replace it with DeepFilterNet3 (MIT/Apache, full-band 48 kHz, 10 ms hop matching kyoko's frame) as the default, and offer RNNoise 0.2 (BSD, the 2024 model) as a low-CPU choice. The setting becomes a choice like Discord's: **High quality** (DeepFilterNet3, default), **Low CPU** (RNNoise 0.2), **Off**. `nnnoiseless` leaves the tree.

**Blocked by:** none (touches `crates/voice` capture chain; 104 camera send and 113 stream audio also touch `crates/voice`: rebase on whichever lands first)

**Status:** claimed

- [ ] DeepFilterNet3 runs in the capture chain after resampling to 48 kHz and before input volume and the gate; standard model (2-frame lookahead), inference on its own thread or inline if it stays well under the 10 ms budget; measured ms per frame and added latency written in `docs/voice.md`
- [ ] Integration: `deep_filter` is git-only at 0.5.x with tract 0.21 and no release since 2024: vendor or pin it (licence notices kept, `cargo deny` green, no GPL), model weights embedded or shipped beside the exe with a hash check; release exe growth measured and stated
- [ ] RNNoise 0.2 (xiph/rnnoise, built with `cc` under MSVC, weights fetched by hash at build time or vendored) as Low CPU; `nnnoiseless` removed
- [ ] Setting `voice.noise_suppression` becomes `high_quality` / `low_cpu` / `off`, default `high_quality` (no migration: nobody uses kyoko yet); the Voice page shows the three choices; switching applies live without reconnecting
- [ ] Session tests (`AudioIo::Manual`): public-domain or generated speech mixed with noise at 0, 5 and 10 dB through the pipeline; after delay alignment, SI-SDR improves by a set margin, noise-only stretches drop by at least 20 dB, speech level stays within 3 dB; the same for Low CPU with its own margins; Off passes audio through
- [ ] If CPU per frame is too high on this machine (say over 3 ms), stop and report numbers instead of shipping it as default
- [ ] `THIRD_PARTY_NOTICES.md` gains DeepFilterNet, tract, RNNoise (and the missing libopus entry from 97); `docs/voice.md` updated; settings capture checked by eye

## Research (2026-10-29, orchestrator's research agent; clones in `nguyenvu/.research/noise/`, untrusted, read only)

- Krisp (Discord's) removes non-voice background noise on your side only; it is proprietary. Removing other people's voices (Krisp's separate voice cancellation) has no permissive open model without an enrollment sample: out of scope.
- DeepFilterNet3: PESQ 3.17 on VoiceBank-DEMAND, RTF 0.19 on one i5-8250U thread via Rust/tract, 960 FFT / 480 hop, 2 lookahead frames (~30 ms algorithmic, paper says 40 ms overall). https://arxiv.org/html/2305.08227, https://github.com/Rikorose/DeepFilterNet. Model ~8.6 MB fp32 ONNX. Low-latency variant has 0 lookahead but is 39 MB. Estimate 1-2 ms per frame, +3-6 MB exe for tract.
- RNNoise 0.2 (April 2024): new 32-band model, AVX2 paths, public training data, a "little" model. https://github.com/xiph/rnnoise. Estimate well under 0.5 ms per frame, +10 ms latency.
- Rejected: Meta Denoiser (non-commercial), GTCRN and FRCRN (16 kHz only), PercepNet (no weights), NSNet2 (weights licence unclear), DPDFNet (weights licence not stated; revisit later).
