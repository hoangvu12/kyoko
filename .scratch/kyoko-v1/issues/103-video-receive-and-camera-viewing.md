# 103 — Video receive surface and camera viewing

**What to build:**
- Port the video RTP depacketizer, MF H.264 decode and the receive path.
- Op 12 announcements and per-SSRC Media Sink Wants with `pixelCounts` from tile sizes.
- An `apps/desktop` video surface feeding `image_signal`.
- Spike first: frame upload cost in zgx.

Plan ticket V7 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 101

**Status:** resolved

- [x] Session test: a scripted member's op 12 plus H264 fixture frames (DAVE on) produce decoded frames for that user; a PLI is sent on loss; op 13 ends the tile
- [x] Wants: a focused tile asks for 100, a thumbnail 50, off-screen 0
- [x] Measured CPU and GPU upload for 1 and 4 tiles at 720p30; the frame rate is capped if needed; written down in `docs/media.md`

## Answer

Merged to main as `985c783`..`6659a02` (five commits; worker `15c53ff5`, lane A; full verify re-run by the orchestrator: all green; `tls-capture --compare chrome-156` 0 and 0; kyoko-only captures `voice-video` (new) and `voice-call` checked by eye). Docs: `docs/voice.md` ("Video", fixture command), `docs/media.md` ("Video", the spike), `docs/ui.md`, `docs/architecture.md`, `docs/parity.md`.

- Root cause: no video receive path: payload type 101 dropped, op 12 ignored, op 15 only `{"any":100}`, no PLI, no H.264 in DAVE.
- dave: `Codec::H264` with oxidcord's `codec.rs` (NAL start codes and slice headers unencrypted; reseal when ciphertext would look like a start code).
- voice `video/` (oxidcord port): depacketizer, Media Foundation decoder on its own thread (sets up COM itself, fine when cpal already did); op 12 maps streams to users, op 13 or a fresh connection ends them; PLI on loss (none on a clean start); op 15 wants recomputed on view or stream changes.
- session: `Command::SetVideoViews`, `VideoFeed` (converts only frames for on-screen tiles, at tile size, never above source). bridge `CallTile.video` becomes camera on op 12. desktop: host image `video/<user id>`, tiles report size and focus via `voice.video_views`; "Video unavailable" when Windows has no H.264 decoder (probed off the UI thread).
- Wants recorded: `{"9102":100,"9202":50,"9302":0,"any":100,"pixelCounts":{"9102":921600,"9202":57600}}`; closing the view sends 0 for all.
- Measured (dev profile, median of 3, other lanes building): no video 3.4% of a core; 1 tile capped (~630x354, 15 fps) 28.5% CPU, 2.4% GPU; 4 tiles capped (~525x295, 15 fps) 66%, 4.2%; uncapped 720p30 79% (1) and 184% (4). Per 720p frame: decode 6.6 ms, NV12->RGBA 5.3 ms (lookup tables; was 17 ms), zgx upload 8-10 ms CPU on the render thread (new texture per frame, CPU premultiply). Caps: thumbnails 15 fps, focused 30.
- Mockcord: scripted op 11/12/13, `ScriptedCamera` (fixture as DAVE frames over RTP), recorded client RTCP, `--cameras <tiles>:<fps>[:<clip>]`. Fixture `tools/mockcord/fixtures/video/camera.h264` (105 KB, 640x360 15 fps, ffmpeg `h264_mf`); `*.h264 binary` in `.gitattributes`. `crates/voice/examples/video_cost` repeats the measurement.
- Tests: `video::a_members_camera_decodes_through_dave_asks_for_a_keyframe_on_loss_and_op_13_ends_it`, `video::wants_ask_100_for_a_focused_tile_50_for_a_thumbnail_and_0_off_screen`, a Luau camera-tile test, a bridge op 12 test.
- Deviations: the test's op 13 comes from a plain scripted frame (`disconnect_as`; the DAVE harness can't remove its only member); RTX (102) not repaired; measured in dev profile.
- For later: 104 can reuse `ScriptedCamera`'s packetizer and oxidcord's encoder; 106 can use `VideoFeed` and `video/<id>`; 91 can model frames on the same host-image path. Decode now dominates: MF hardware decode via a D3D11 device manager plus an NV12 zgx surface would cut most of it. Mockcord senders must use `MissedTickBehavior::Burst` (a 33 ms tokio interval fires ~47 ms on Windows).
