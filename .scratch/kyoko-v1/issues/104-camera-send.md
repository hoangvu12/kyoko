# 104 — Camera send

**What to build:** Port the WGC/MF camera capture and MF H.264 encode with two simulcast layers (100, 50). Op 12 announce and clear, `self_video` in op 4, honouring server wants, and a camera preview in settings.

Plan ticket V8 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 103

**Status:** resolved

- [x] Session test (fake capture source): Camera on sends op 4 `self_video: true`, op 12 with the Ready-assigned SSRCs for both layers, and DAVE-encrypted H264 with playout-delay; wants of 0 pause a layer; Camera off clears
- [x] MF unavailable (N edition) shows "video unavailable" without breaking audio

## Answer

Merged to main as `119d584` and `74b1738` (worker `1e626948`, lane B; full verify re-run by the orchestrator: all green, tls-capture 0/0 against chrome-156; kyoko-only captures `voice-camera`, `settings-voice-camera` (new), `settings-voice-push-to-talk`, `voice-video` checked by eye). Docs: `docs/voice.md` ("Camera"), `docs/media.md`, `docs/ui.md`, `docs/parity.md`, `docs/architecture.md`.

- Root cause: kyoko offered layers 100 and 50 in identify but ignored the SSRCs Ready assigned; no camera capture, encoder or send path; op 4 always sent `self_video: false`; op 15 wants and RTCP feedback were dropped; the camera button was a disabled placeholder.
- Send: turning the camera on captures it, encodes two simulcast layers (rid 100 and 50) with the Media Foundation H.264 software encoder (low-latency mode set before the media types, or it holds 16 frames), DAVE-encrypts and sends them on Ready's SSRCs with playout-delay (ext id 6) and RID (ext id 11). op 12 announces the camera and clears it on Camera off; op 4 carries `self_video`. Wants of 0 pause a layer (op 12 marks it inactive); wanting it again resumes on a keyframe. PLI/FIR gets a keyframe.
- Code: in `crates/video` beside 91's decoder: `encode.rs` (`H264Encoder`, `EncoderConfig`, `VideoCodecs`), `media_foundation_encoder.rs`, `mf_send.rs`, `camera.rs` (`CameraSource`), `capture.rs` (`VideoSource`, `cameras`, `open_camera`, `TestPattern`), `scale.rs`, `preview.rs` (`CameraPreview`), `annex_b.rs`, `VideoQuality`. In `crates/voice` (network side): `video/send.rs` (the `video-send` thread), `outgoing.rs` (layers, wants, queue), `packetize.rs` (FU-A, playout-delay, RID), op 12 in the supervisor. `video::available()` now means decode and encode H.264 (desktop probe only).
- UI: the call view's camera button toggles it and your own tile shows your camera; Voice & Video gains "Video Settings" (Camera select, Test Video / Stop Video 16:9 preview). Under `--mock` the call and preview use a moving test pattern. Without an encoder the button is disabled and the preview says "Video unavailable".
- Tests: session `camera::the_camera_announces_both_layers_sends_dave_h264_honours_wants_and_clears`, `camera::without_an_h264_encoder_the_camera_says_video_unavailable_and_audio_goes_on`, a bridge own-tile test, Luau tests for the button, own tile and settings rows. Tests must keep `session::CameraIo::Pattern`; `VideoCodecs::Missing` simulates Windows N. Mockcord: no longer echoes a client's own video (reassembles and records it), relays op 12; helpers `sink_wants`, `request_keyframe`, `VoiceRecording::video` (`VoiceVideoFrame` with `rid()`, `extension()`, `is_keyframe()`).
- Deviations: software encoder only (the NVIDIA and Intel MFTs produced nothing without a D3D11 device manager and the async path); encoder CPU not measured; no RTX/NACK, RTCP sender reports, preview mirroring, voice panel camera button or quality picker (`video_preset` 0/1/2 = 720p/480p/360p); a camera device change applies on the next camera start.
- For later: 107 (going live) can reuse the `video-send` thread, Ready's layer list and the op 12 builder; stream connections already get screen-layer SSRCs from Ready (mockcord too). Hardware encode needs a D3D11 device manager plus the async MFT path. Flake seen twice under load: `timeline::video_tests::a_video_attachment_plays_inline_with_discords_controls_and_pauses_off_screen` (added to rules.md); once in headless-run: "redrawing the timeline asked ShowMeYourName again".
