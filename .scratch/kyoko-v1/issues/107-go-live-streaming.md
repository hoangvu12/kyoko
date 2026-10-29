# 107 — Go Live streaming

**What to build:** Op 18 Create Stream, a stream connection identifying with one `screen` layer at 100, 105's capture encoded and sent, pause (op 22), and stop (op 19).

Plan ticket V11 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 105, 106

**Status:** resolved

- [x] Session test (fake capture): Go Live sends op 18 with type `guild`, connects, announces op 12 and sends encrypted H264; viewers' wants are honoured; Stop sends op 19 and closes only the stream connection
- [x] The LIVE badge shows on your own row and in the call view

## Answer

Merged `47da300` (lane B, worker `8f2a4682`), verified by the orchestrator: fmt, clippy, build, test --workspace, xtask test, headless-run, deny, luau0-src 0.22.0+luau740 green; tls-capture against chrome-156 0/0 differences; kyoko-only captures `voice-going-live` (new), `voice-call`, `voice-go-live-preview` checked by eye.

- Session `crates/session/src/task/go_live.rs` (ported from oxidcord streams.rs share role): `Command::GoLive { source: ScreenTarget, quality }`, `StopGoLive`, `SetStreamPaused(bool)`. Op 18 keys in Discord's order (type guild/call, guild_id, channel_id, preferred_region = first `geo_ordered_rtc_region`); the gateway now sends op 18 and op 22 as structs (the old `json!` sorted keys). On STREAM_CREATE + STREAM_SERVER_UPDATE for your key a `ConnectionKind::Stream` connection opens: one `screen` layer, DAVE group `rtc_server_id - 1`, muted, deafened, `AudioIo::Manual` (no device). Capture (`open_screen`, or the test pattern under `CameraIo::Pattern` / `--mock` / tests) goes through 104's video-send via `VoiceConnection::start_screen`. Op 12 on a stream connection says `type: screen`. Wants (op 15) pause/resume the layer like the camera. Op 22 pauses encode and send; resume starts on a keyframe. Stop sends op 19 and closes only the stream connection; leave, move or a fresh gateway ends it. STREAM_DELETE shows a toast; a closed window raises `VoiceEvent::VideoSourceEnded` ("The window you picked was closed."); 15 s timeout. `Session::share_connection()` for tests; streams.rs dispatches by generation to watch or share.
- UI: the picker's Go Live is enabled once the preview runs (`voice.go_live_start` {id, resolution, fps}); bitrate scales from 2.5 Mbps at 720p30, clamped 1-8 Mbps. The call view's screen button stops the stream ("Stop Streaming", `voice.stop_go_live`). Own LIVE on the sidebar row and own tile comes from `self_stream` or `VoiceSession::screen_share` (`voice.controls.live`).
- Mockcord: op 18 opens your own MockStream (own rtc ids and DAVE group), answers CREATE/SERVER_UPDATE (token `mock-voice-token-stream-<id>`), broadcasts `self_stream` true; op 19 removes it.
- Tests: `go_live::going_live_sends_dave_h264_on_one_screen_layer_honours_wants_pauses_and_stops`, `go_live::going_live_needs_a_call_and_leaving_the_call_ends_the_stream`, `ui::scripts::voice::going_live_shows_your_live_badge_and_the_screen_button_stops_it`.
- Not done (ticket 118): stream audio send (the stream connection is `AudioIo::Manual`), a pause trigger (Discord pauses when the shared window is minimised), a preview tile of your own stream, swapping source without op 19 + op 18, Nitro gating. Hardware encode still pending from 104. The real WGC path was not hand-run in this ticket (105 did once).
