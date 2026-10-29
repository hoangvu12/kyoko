# 116 — Echo cancellation and automatic gain control

**What to build:** Follow-up from 100 and 115. Kyoko has no echo cancellation (the Voice page says so) and no automatic gain control, so people hear themselves when a kyoko user is on speakers. Discord's desktop media engine is built on WebRTC's native library. Add WebRTC's audio processing through `sonora` (pure-Rust port of WebRTC APM: AEC3, AGC2, high-pass filter; BSD-3; crates.io 0.2.0), with Echo Cancellation and Automatic Gain Control toggles on the Voice page, both on by default.

**Blocked by:** 115 (same capture chain; order below)

**Status:** claimed

- [ ] Capture order: 48 kHz → high-pass + AEC3 → noise suppression (115) → AGC2 → input volume → gate → Opus. WebRTC's own noise suppressor stays off
- [ ] Render reference: every mixed 10 ms output frame (after per-user volumes, output volume and deafen, right before the device) goes to `process_render_f32`; the device delay is estimated from cpal's buffer latency and set with `set_stream_delay_ms`; the mic test path gets the same treatment
- [ ] Settings `voice.echo_cancellation` and `voice.auto_gain_control` (default on), applied live; the Voice page's "not available" note is replaced by the toggles
- [ ] Session tests (`AudioIo::Manual`): mixer output fed back into the input with a delay and gain gives echo return loss enhancement of at least 20 dB with AEC on and none with it off; near-end speech during far-end playback stays within a set level; AGC brings a quiet and a loud speaker to within a set range
- [ ] Vet `sonora` first (one maintainer, described as an AI-assisted port): read its tests and CI, check it against the C++ suite claim, pin the version, `cargo deny` green. If it doesn't hold up, stop and report options (Windows 11 `IAcousticEchoCancellationControl` with raw WASAPI, or the C++ APM) instead of shipping it
- [ ] CPU per frame measured (research says ~20 µs on a 5950X); `docs/voice.md` and `THIRD_PARTY_NOTICES.md` updated; settings capture checked by eye

## Research (2026-10-29, orchestrator's research agent)

- `sonora`: https://github.com/dignifiedquire/sonora (README: C++ test suite passes; CI on Windows x86_64; `BENCHMARKS.md` 17.8 µs per 10 ms frame at 48 kHz mono for the full chain).
- tonarino `webrtc-audio-processing` 2.1 needs meson/ninja and has no MSVC path in `build.rs`: poor fit.
- Windows 11 22621+ echo cancellation with a chosen reference device: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iacousticechocancellationcontrol (needs WASAPI instead of cpal).
- Limit: sounds from other apps are not in the render reference, so they aren't cancelled (same as any in-app AEC).
