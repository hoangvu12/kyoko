# 124 — Discord closes the gateway with 4002 "Error while decoding payload"

**What to build:** In the user's real run on 2026-10-30 (`nguyenvu\.orchestrator\real-run\stdout-4.txt`, main `7d9f340`), Discord closed kyoko's gateway with 4002 "Error while decoding payload." three times in ten minutes. Each time kyoko's resume got InvalidSession and it identified again. The runs of 2026-10-27 (stdout-2, stdout-3) had no closes, so something merged since then sends a payload Discord can't decode. The log has only event names, no payloads:

- Close 1: about 1 s after a MESSAGE_ACK dispatch, about 8 s after a VOICE_STATE_UPDATE, while the user was clicking around.
- Close 2: 0.3 s after READY, READY_SUPPLEMENTAL and SESSIONS_REPLACE of the fresh session, before anything else arrived (so something kyoko sends on its own right after READY).
- Close 3: 80 ms after a GUILD_MEMBERS_CHUNK dispatch, after a run of GUILD_MEMBER_LIST_UPDATEs.
- A third fresh session in between went through with no close (voice joined fine).

Find what kyoko sends that Discord can't decode, fix it, and make it impossible to miss next time. Leads to check first (merged since 2026-10-27; `git log --since=2026-10-27T12:00 -- crates/gateway crates/session`): `d6245b6` moved the gateway onto a wreq websocket (frame type text vs binary, fragmentation of large frames, permessage-deflate on outgoing frames, zlib-stream); `102cd95` resends presence after a fresh session (op 3 shape, `activities` from plugins like game-activity-toggle); `3a21caa` loads own roles from READY members carrying only `user_id` (op 8 Request Guild Members after a chunk: `user_ids` count limit of 100, duplicates, empty lists, `guild_id` shape); op 37 guild subscriptions bulk; op 4 with `preferred_region(s)`/`flags` (98, 108). Compare each outgoing opcode with docs.discord.food and with what the web client sends (`docs/gateway.md`).

**Blocked by:** none

**Status:** claimed (lane A)

- [ ] Root cause found and stated in plain words, with evidence (a mockcord reproduction that fails the same way)
- [ ] Fixed; a session test against mockcord proves the bad payload is no longer sent, and mockcord now rejects that payload with close 4002 the way Discord does (so a regression fails a test)
- [ ] Debug logging of every outgoing gateway message's opcode name, frame type and byte length, and the top-level field names with array lengths (never values, never the token), so a real run shows what was sent just before any close; `docs/gateway.md` says so
- [ ] Any other outgoing opcode that could hit the same problem is checked and listed (fine / fixed)
- [ ] tls-capture chrome-156 unchanged (0/0 differences)
