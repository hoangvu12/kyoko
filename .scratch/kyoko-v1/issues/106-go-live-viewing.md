# 106 — Go Live viewing

**What to build:** Port `streams.rs` viewing: Watch Stream (op 20), STREAM_CREATE and STREAM_SERVER_UPDATE, a second voice connection to the stream server (DAVE group `rtc_server_id - 1`), the stream tile and viewer, Stop Watching (op 19), and STREAM_DELETE reasons.

Plan ticket V10 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 103

**Status:** open

- [ ] Session test: watching sends op 20, connects to the stream endpoint with `server_id` = `rtc_server_id`, joins the DAVE group `rtc_server_id - 1`, and decodes the fixture stream; the parent voice connection is unaffected
- [ ] `stream_full` and `user_requested` deletes end the view with the right text; leaving voice ends every stream view
