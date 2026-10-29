# 124 — Discord closes the gateway with 4002 "Error while decoding payload"

**What to build:** In the user's real run on 2026-10-30 (`nguyenvu\.orchestrator\real-run\stdout-4.txt`, main `7d9f340`), Discord closed kyoko's gateway with 4002 "Error while decoding payload." three times in ten minutes. Each time kyoko's resume got InvalidSession and it identified again. The runs of 2026-10-27 (stdout-2, stdout-3) had no closes, so something merged since then sends a payload Discord can't decode. The log has only event names, no payloads:

- Close 1: about 1 s after a MESSAGE_ACK dispatch, about 8 s after a VOICE_STATE_UPDATE, while the user was clicking around.
- Close 2: 0.3 s after READY, READY_SUPPLEMENTAL and SESSIONS_REPLACE of the fresh session, before anything else arrived (so something kyoko sends on its own right after READY).
- Close 3: 80 ms after a GUILD_MEMBERS_CHUNK dispatch, after a run of GUILD_MEMBER_LIST_UPDATEs.
- A third fresh session in between went through with no close (voice joined fine).

Find what kyoko sends that Discord can't decode, fix it, and make it impossible to miss next time. Leads to check first (merged since 2026-10-27; `git log --since=2026-10-27T12:00 -- crates/gateway crates/session`): `d6245b6` moved the gateway onto a wreq websocket (frame type text vs binary, fragmentation of large frames, permessage-deflate on outgoing frames, zlib-stream); `102cd95` resends presence after a fresh session (op 3 shape, `activities` from plugins like game-activity-toggle); `3a21caa` loads own roles from READY members carrying only `user_id` (op 8 Request Guild Members after a chunk: `user_ids` count limit of 100, duplicates, empty lists, `guild_id` shape); op 37 guild subscriptions bulk; op 4 with `preferred_region(s)`/`flags` (98, 108). Compare each outgoing opcode with docs.discord.food and with what the web client sends (`docs/gateway.md`).

**Blocked by:** none

**Status:** resolved

- [x] Root cause found and stated in plain words, with evidence (a mockcord reproduction that fails the same way) — from timing and mock repro; not yet confirmed on real traffic
- [x] Fixed; a session test against mockcord proves the bad payload is no longer sent, and mockcord now rejects that payload with close 4002 the way Discord does (so a regression fails a test)
- [x] Debug logging of every outgoing gateway message's opcode name, frame type and byte length, and the top-level field names with array lengths (never values, never the token), so a real run shows what was sent just before any close; `docs/gateway.md` says so
- [x] Any other outgoing opcode that could hit the same problem is checked and listed (fine / fixed)
- [x] tls-capture chrome-156 unchanged (0/0 differences)

## Answer

Merged to main as `335bfc9` (worker `b2eb0f49`, lane A; full verify re-run by the orchestrator: all green, `xtask test` green on rerun after the known `voice_settings::high_quality_noise_suppression...` load flake; `tls-capture --compare chrome-156` 0 and 0 on https and wss). Docs: `docs/gateway.md` ("Outgoing payloads (v1 124)", "Pending memberships").

- Root cause (likely, not confirmed on real traffic): transport ruled out (plain unfragmented text frames, no permessage-deflate); every outgoing op from 299 session tests matched docs.discord.food and stayed under 2.2 KB. Three closes came one round trip after a GUILD_MEMBERS_CHUNK (when the next queued op 8 goes out), one at READY+273 ms; op 8/op 37 code hadn't changed since 10-27, so new data reached it. Two candidates, both fixed: (a) member ids parsed from `<@...>` text that aren't real snowflakes (0 or above i64::MAX; the markdown parser accepts any u64) went into op 8 `user_ids`; (b) op 8 / op 37 for a guild where the user's membership is pending or onboarding isn't finished.
- `Snowflake::is_valid` (0 < id <= i64::MAX); mentions filtered in state; the member scheduler drops invalid ids and empty searches, and drops (not retries) a request the handle refuses.
- `State::is_membership_pending` (own member `pending`, or ONBOARDING guild with started-but-not-completed flags): no op 8 and no op 37 for that guild (channel open, SubscribeGuild, SetMemberViewport, SearchMembers, backfill skipped; queued requests dropped when due); a GUILD_MEMBER_UPDATE ending it resubscribes the open channel. Deviation: no member list or typing there until onboarding completes; the web client probably still subscribes. If a real run shows op 37 wasn't the culprit, relax to `pending` only.
- `GatewayHandle` refuses frames over 15 KiB and op 8 with bad/duplicate ids, more than 100 ids, a bad limit, or a nonce over 32 bytes (`SendError::Invalid`). Op 3 sends a custom-status emoji only with a name.
- Logging: one debug line per send, `gateway send opcode=.. frame="text" bytes=.. fields=name[len],...`, names and lengths only. A real run needs `--log info,gateway=debug` (the desktop reads `--log`, not RUST_LOG).
- Mockcord `strict.rs` closes 4002 "Error while decoding payload." on oversize frames, bad op 8/op 37/op 3/op 4 shapes and pending guilds; the whole suite passes against it. Tests: `payloads::mentions_of_impossible_ids_never_reach_a_member_request`, `payloads::a_pending_membership_asks_for_no_members_or_member_list` (both fail on the old code).
- For 132: the Onboarding 403 comes from the desktop's `follow_gate` fetching `GET /guilds/{id}/onboarding` when a gated guild is selected; the composer's `may_send` only drops typing, it doesn't refuse input.
