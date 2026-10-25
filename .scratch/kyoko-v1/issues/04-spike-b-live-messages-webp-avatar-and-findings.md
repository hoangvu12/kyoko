# 04 — Spike B: live messages, WebP avatar and findings

**What to build:** Opening a channel in the spike window shows new messages arriving live. One WebP avatar is fetched and decoded in Rust and drawn by zgx. The spike's findings are written up so later tickets can adjust.

**Blocked by:** 03 (Spike A: zgx window with a live server/channel list)

**Status:** resolved

- [ ] Opening a channel shows its recent messages and adds new ones live, on both mockcord and real Discord (mockcord: done; real Discord: recent messages load, but no live message arrived during either run — confirm on the next real run with someone posting)
- [x] One WebP avatar is fetched, decoded in Rust and shown as a host-supplied image
- [x] The capability bits and the shape of IDENTIFY and the REST headers are compared against real traffic, and the result is recorded
- [x] Findings are written down: build times, memory, zgx issues, bridge behaviour, and anything that changes later tickets

## Answer

Merged to main as `d8e7dc5..e7e1c5e` (six commits). Findings: `docs/spike.md`. The changes it implies for later tickets are recorded on main in `3ca49a9` (new ticket 59 and comments on 18, 20, 25, 29, 35, 43, 45).

- `bridge::Timeline` projects the open channel (id, author, plain text, timestamp); `--chatter N` makes mockcord post into the open channel; `--open-busiest` opens the most recently active text channel. The bridge now depends on `media`. Bridge tests are one binary under `crates/bridge/tests/bridge/` (8 tests) with a shared harness.
- Avatar: `.webp?size=N` via `media`, decoded in Rust, shown with `ScriptHost::image("avatar", ..)`; mockcord serves WebP avatars. 72–125 ms on mock, 200–300 ms from the real CDN.
- Real runs (2, look only, about 4–5 min each): READY on v9, no captcha/401/close code; 8 servers; 50 messages loaded; no live message in the open channel; no `auth_token` in READY; scraped build 649389 (now the fallback); cookies `__dcfduid`, `__sdcfduid`, `_cfuvid`; READY keys `qos_token` and `subdivision_code` not in the docs; 60–76 member-list updates in about 4 minutes. Credential Manager unchanged.
- Comparison with the web client (discord.food / userdoccers, read 2026-10-25): Chrome 152 vs stable 156; capabilities keep 509; presence `"unknown"`/`since: 0`; `client_state.api_code_version`; IDENTIFY key order, drop `large_threshold`; `launch_signature`; rotate heartbeat session id; `installation_id` in IDENTIFY. All in ticket 59.
- Measurements: dispatch-to-UI median 1.4 ms (max 4 ms), none dropped or duplicated. Debug exe 57.7 MB, release 51.2 MB (release build 7m28s). Memory: debug mock idle 189 MB, real idle 147–168 MB; release mock idle 197.7 MB at 0.05% of a core, active 188.9 MB at 0.21%. The window/GPU side dominates memory, not code size.
- zgx: about one close in three leaves the process running (mock too); the scratch dir is now removed as soon as the session stops; root cause for ticket 20. zgx gives the app no focus/minimise signal, so `SetViewing` isn't sent yet (ticket 25). zgx `rounded` doesn't clip images (ticket 18). Host image and shared-value names are global (ticket 45).
