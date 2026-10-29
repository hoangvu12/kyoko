# 116 — Echo cancellation and automatic gain control

**What to build:** Follow-up from 100 and 115. Kyoko has no echo cancellation (the Voice page says so) and no automatic gain control, so people hear themselves when a kyoko user is on speakers. Discord's desktop media engine is built on WebRTC's native library. Add WebRTC's audio processing through `sonora` (pure-Rust port of WebRTC APM: AEC3, AGC2, high-pass filter; BSD-3; crates.io 0.2.0), with Echo Cancellation and Automatic Gain Control toggles on the Voice page, both on by default.

**Blocked by:** 115 (same capture chain; order below)

**Status:** resolved

- [x] Capture order: 48 kHz → high-pass + AEC3 → noise suppression (115) → AGC2 → input volume → gate → Opus. WebRTC's own noise suppressor stays off
- [x] Render reference: every mixed 10 ms output frame (after per-user volumes, output volume and deafen, right before the device) goes to `process_render_f32`; the device delay is estimated from cpal's buffer latency and set with `set_stream_delay_ms`; the mic test path gets the same treatment
- [x] Settings `voice.echo_cancellation` and `voice.auto_gain_control` (default on), applied live; the Voice page's "not available" note is replaced by the toggles
- [x] Session tests (`AudioIo::Manual`): mixer output fed back into the input with a delay and gain gives echo return loss enhancement of at least 20 dB with AEC on and none with it off; near-end speech during far-end playback stays within a set level; AGC brings a quiet and a loud speaker to within a set range
- [x] Vet `sonora` first (one maintainer, described as an AI-assisted port): read its tests and CI, check it against the C++ suite claim, pin the version, `cargo deny` green. If it doesn't hold up, stop and report options (Windows 11 `IAcousticEchoCancellationControl` with raw WASAPI, or the C++ APM) instead of shipping it
- [x] CPU per frame measured (research says ~20 µs on a 5950X); `docs/voice.md` and `THIRD_PARTY_NOTICES.md` updated; settings capture checked by eye

## Research (2026-10-29, orchestrator's research agent)

- `sonora`: https://github.com/dignifiedquire/sonora (README: C++ test suite passes; CI on Windows x86_64; `BENCHMARKS.md` 17.8 µs per 10 ms frame at 48 kHz mono for the full chain).
- tonarino `webrtc-audio-processing` 2.1 needs meson/ninja and has no MSVC path in `build.rs`: poor fit.
- Windows 11 22621+ echo cancellation with a chosen reference device: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iacousticechocancellationcontrol (needs WASAPI instead of cpal).
- Limit: sounds from other apps are not in the render reference, so they aren't cancelled (same as any in-app AEC).

## Answer

Merged `f3e8868` (lane C, worker `7d683bcd`), verified by the orchestrator: fmt, clippy, build, test --workspace, xtask test, headless-run, deny, luau0-src green; tls-capture chrome-156 0/0; kyoko-only captures `settings-voice`, `settings-voice-processing` checked by eye (both switches on with notes).

- `crates/voice/src/processing.rs` (`voice::Processor`, `voice::Processing`): two sonora (`=0.2.0`, BSD-3) instances at 48 kHz mono, full-band: HPF + AEC3, then AGC2 (adaptive digital + limiter, no analog). Capture order 48 kHz -> HPF+AEC3 -> NS (115) -> AGC2 -> input volume -> meter/gate -> Opus; WebRTC NS off. HPF is tied to the Echo Cancellation switch.
- Render reference: every mixed frame (after per-user volumes, output volume, deafen; silence included) downmixed to mono, two 10 ms chunks to `process_render_f32`. Delay = queued playback + cpal input/output latency from callback timestamps (`Rings::echo_delay`), 0 under Manual. The mic test uses its own played-back voice as reference. Stream connections never process (`set_processing` is a no-op for `ConnectionKind::Stream`).
- Settings `voice.echo_cancellation`, `voice.auto_gain_control` (default on, live via `ApplyVoiceSettings`; turning one off drops its state). `VoiceStats` has `frames_echo_cancelled`, `frames_gain_controlled`. In `AudioIo::Manual`, `take_output` now returns every mixed frame (silent ones with empty `sources`; the tap holds up to 3000 frames).
- Tests `crates/session/tests/session/voice_processing.rs`: echo fed back 65 ms later at 0.5 gain, median per-second ERLE over 4-10 s ~65-71 dB on, 0.3 dB off; near-end speech within 6 dB; AGC brings -45 and -15 dBFS speech into -30..-10; mic test quiet speech plays back at -26 dB. One loaded run gave a single-window ERLE of 20 dB, hence the median.
- CPU (release, per 10 ms): whole chain mean 295 us, p99 833 us (about 3% of a core); full-band kept (32 kHz only saved ~60 us). Release exe 118.4 MB (+1.0 MB).
- sonora vetting: ~720 Rust tests, CI on Windows x86_64 among others, daily fuzzing, Rust-vs-C++ comparisons. The README's "full C++ test suite passes" overstates it: only the ~77 APM-level C++ tests run through Rust (component tests still test C++ classes; bit-exact `ApmTest.Process` disabled), on Linux/macOS only. One maintainer, "AI assisted" port. Bump when 0.3 ships (unreleased AEC3 render-buffer underrun fix).
- Gaps: stream audio being watched plays from the stream connection's own mixer, so it is not in the AEC reference and echoes into the call on speakers (added to 118); other apps' sounds aren't cancelled; real-device delay estimation is untested by automation (AEC3 estimates delay too). AEC isn't closed-loop tested in the mic test (no clock there).
