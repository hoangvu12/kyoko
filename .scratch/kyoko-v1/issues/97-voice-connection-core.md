# 97 — Voice connection core on mockcord (audio, DAVE off)

**What to build:** Port `crates/voice` without video. Signalling on `net::ws` (v9, web heartbeat, `rtc_connection_id`, two simulcast layers in identify), UDP transport with both rtpsize modes, Opus via libopus, jitter, mixer, resample and gate. Mockcord's voice server moves onto TLS and records frames.

Plan ticket V1 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 96

**Status:** resolved

- [x] The voice upgrade presents `UPGRADE_ORDER` and the Chrome wss ClientHello (same check as the gateway's, `tls_fingerprint`); no Cookie
- [x] Identify is v9 with `channel_id`, `video: true`, layers 100 and 50, `max_dave_protocol_version: 1`; heartbeat every `min(interval, 5000)` ms; resume carries `seq_ack` and `channel_id`
- [x] Select protocol picks AES-GCM when offered and XChaCha otherwise; both seal and open against mockcord's echo
- [x] A 1 kHz tone fed to the manual input comes back decoded from the echo SSRC; 5 silence frames follow the end of speech; op 5 is sent before media
- [x] Close 4006/4009 starts fresh, 4015 and network loss resume, 4014/4017 stop with a reason

## Answer

Merged to main as `1d5d8a2` (worker `9780f3c3`, lane B; checks re-run by the orchestrator: all green; `tls-capture --client wreq --h2 --compare chrome-156` 0 differences). Docs: new `docs/voice.md`, `docs/architecture.md`, `docs/fingerprint.md` ("Voice"), `docs/testing.md`.

- `crates/voice` (`kyoko-voice`) ported from oxidcord without video or DAVE: signalling on `net::ws`, UDP with IP discovery behind a thin `Transport` trait, `aead_aes256_gcm_rtpsize` / `aead_xchacha20_poly1305_rtpsize`, libopus (64 kbps fixed), jitter buffer, mixer, resampler, voice gate, cpal devices; RNNoise ported, off by default. The build trial moved to `examples/voice_trial/`; openmls, Media Foundation and Graphics Capture are dev-dependencies until 99 and video; openh264 an optional feature.
- API: `VoiceConnection::connect(VoiceParams, VoiceOptions)`; `AudioIo::Manual` with `feed_input` / `take_output` (frames tagged with SSRC); `VoiceEvent::{Connected { ssrc, mode }, Resumed, Reconnecting { code, resume }, Stopped(StopReason)}`; a `FrameTransform` slot for DAVE.
- Verified (13 tests in `crates/voice/tests/voice`, mockcord on TLS): voice ClientHello 0 differences from `chrome-156-wss`, ALPN `http/1.1`, `GET /?v=9`, `UPGRADE_ORDER`, no Cookie; identify `channel_id`, `video: true`, layers 100/50, `max_dave_protocol_version: 1`; Select Protocol with UUID `rtc_connection_id` and echoed `experiments`; heartbeat `min(interval, 5000)`, first one period after Hello; resume `server_id, channel_id, session_id, token, seq_ack`; AES-GCM preferred, XChaCha when alone; 1 kHz tone back from the echo SSRC at 1000 ± 40 Hz; 5 silence frames; op 5 before media; 4006/4009 identify again, 4015 and socket loss resume, 4014 `Stopped(Disconnected)`, 4017 `Stopped(E2eeRequired)`.
- Mockcord voice server: TLS under the `tls` feature (`Server::voice().root_pem()`), plain `ws://` in the app's `--mock`; records hellos, upgrade requests, frames, closes and decrypted RTP; echo from SSRC + 1; controls `set_modes`, `set_heartbeat_interval`, `close_sockets(code)`, `drop_sockets()`, `speak_as(user, ssrc, speaking)`; closes 4004/4006/4016.
- Deviations: tests at the voice crate seam (no `Command::JoinVoice` yet); Select Protocol still offers H264 101/102 as oxidcord does, incoming video dropped.
- For 98: add `voice = { path = "crates/voice", package = "kyoko-voice" }` to `[workspace.dependencies]`; `VoiceParams` from VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE (guild or none, channel, user, gateway session id, token, endpoint), the identity's user agent and locale in `VoiceOptions`; leaving calls `disconnect()`. For 99: client sends `max_dave_protocol_version: 1` but ignores `dave_protocol_version` and binary frames (mockcord answers 0); implement `FrameTransform`. For 101: `speak_as` scripts op 5; add op 11/13 the same way. `THIRD_PARTY_NOTICES.md` needs libopus and, with DAVE, hpke-rs.
- Known flake seen here and on lane A: `theme_links_are_fetched_over_https_only_without_any_discord_credentials` fails when another process probes new localhost ports (`GET /` with only Host and Connection); a test fix is to ignore requests it didn't ask for.
