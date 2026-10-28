# 106 — Go Live viewing

**What to build:** Port `streams.rs` viewing: Watch Stream (op 20), STREAM_CREATE and STREAM_SERVER_UPDATE, a second voice connection to the stream server (DAVE group `rtc_server_id - 1`), the stream tile and viewer, Stop Watching (op 19), and STREAM_DELETE reasons.

Plan ticket V10 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 103

**Status:** resolved

- [x] Session test: watching sends op 20, connects to the stream endpoint with `server_id` = `rtc_server_id`, joins the DAVE group `rtc_server_id - 1`, and decodes the fixture stream; the parent voice connection is unaffected
- [x] `stream_full` and `user_requested` deletes end the view with the right text; leaving voice ends every stream view

## Answer

Merged to main as `2f0094a`..`d84ff73` (four commits; worker `f784c3df`, lane A; full verify re-run by the orchestrator: green apart from `composer::a_server_gate_points_to_the_official_client_without_a_confirm` timing out under load in `test --workspace` (second time; passed in the same run's `xtask test`; added to rules.md's flake list); `tls-capture --compare chrome-156` 0 and 0; kyoko-only captures `voice-stream`, `voice-stream-watching` checked by eye). Docs: `docs/voice.md` ("Go Live", mockcord notes), `docs/ui.md`, `docs/parity.md`.

- Root cause: stream gateway events were decoded and kept in state, but nothing acted on them: no Watch/Stop Watching, no second connection; the voice core knew only the call's identify and used the channel id as DAVE group; no stream tile; no mock stream server.
- Voice core: `ConnectionKind::Stream` identifies with `server_id` = `rtc_server_id`, `channel_id` = `rtc_channel_id`, the stream token and one layer `{type:"screen", rid:"100", quality:100}`; DAVE group `rtc_server_id - 1`; runs muted and deafened (never sends media).
- Session `streams.rs`: watch sends op 20, connects on STREAM_SERVER_UPDATE, op 15 asks 100; Stop Watching sends op 19 and closes only the stream connection; STREAM_DELETE reasons as toasts (`stream_full` "This stream is full.", `user_requested` "The stream has ended."); leaving voice ends every view; one stream at a time. `StreamState.viewer_ids` decodes leniently.
- UI: `bridge::VoiceCall.streams`; stream tile (avatar, Watch Stream, name pill with screen icon and LIVE) before the streamer's own tile; LIVE on the streamer's tile and sidebar row; watched stream fills the focused tile (letterboxed) with Stop Watching; `video_surface.rs` has two lanes, `video/<id>` and `stream/<id>`, action `voice.stream_views`; tiles focus by `key` (person and stream tiles focus separately); the call view focuses a stream when the session marks it watched.
- Mockcord: voice sessions keyed `server_id:session_id`; DAVE room membership and mail keyed by (channel, user); scripted broadcasts carry an optional channel scope; controls `start_stream`, `fill_stream`, `end_stream`; `--streams <n>`.
- Tests: `streams::watching_a_stream_connects_to_its_server_in_its_own_dave_group_and_leaves_the_call_alone`, `streams::deletes_end_the_view_with_their_reason_and_leaving_voice_ends_every_view`, Luau `a_stream_tile_watches_focuses_reports_its_view_and_stops_watching`.
- Not yet: stream audio isn't played; no full-window stream viewer (focused tile only).
- For later: 107 (going live) needs only op 18 and sending the capture; the voice core already supports `ConnectionKind::Stream`.
