# 14 — Member list ranges and member-request scheduler

**What to build:** The member list of any server can be browsed in full as the user scrolls, using Discord's lazy subscriptions politely.

**Blocked by:** 07 (Gateway send window, heartbeat and reconnect policy)

**Status:** resolved

- [x] Guild subscriptions use op 37, debounced and deduplicated
- [x] Member-list ranges are requested in 100-row buckets around the viewport, not only the first 100
- [x] All member-list update ops are applied: SYNC, INSERT, UPDATE, DELETE and INVALIDATE
- [x] Member requests (op 8) are scheduled and respect rate-limited responses
- [x] Session tests assert the subscriptions and the resulting member-list state

## Answer

Merged to main as `35036c8`.

- Op 37: one planner builds every frame. First entry for a guild: `{"typing":true,"threads":true,"activities":true,"member_updates":false,"members":[],"thread_member_lists":[],"channels":{"<cid>":[[0,99],..]}}`; later entries carry only `channels` (`"<old>":[]` to drop). Debounce 150 ms after the last change, at most 500 ms after the first; several guilds share a frame; unchanged guilds omitted, empty frames never sent. Open guild plus 3 most recent keep a member-list channel. Nothing before READY; a new READY resubscribes only the open guild; RESUMED keeps everything.
- Ranges: `Command::SetMemberViewport`; `[0,99]` + the viewport's buckets + one lookahead bucket toward the nearer edge, capped at 3 (rows 450–470 → `[[0,99],[400,499],[500,599]]`). Opening a channel resets that guild's ranges to the top.
- State: `State::member_list(guild_id) -> Option<&MemberList>` (`id`, `groups`, `slots`, `row_count()`, `rows(first,last)`, `group_count(id)`); `State::member_list_rows(guild_id, first, last) -> Vec<Option<MemberListRow>>` (`None` = not loaded). One list per guild; a new id replaces it only with a SYNC. INVALIDATE clears without resubscribing. GUILD_MEMBER_REMOVE no longer edits the list (Discord sends a list DELETE).
- Op 8 scheduler: `Command::RequestMembers { guild_id, user_ids }`, `Command::SearchMembers`, `Core::request_members(guild_id, ids)`. 50 ms batching, up to 100 ids per request (`presences:false`), one unanswered request per guild, bulk lane, done on the last GUILD_MEMBERS_CHUNK with the nonce or after 10 s. Dedup of queued, in-flight and answered-in-60-s ids. RATE_LIMITED holds the guild for `retry_after` (default 5 s, max 120 s). A new READY requeues unanswered requests.
- API: `GatewayHandle::subscribe_guild` replaced by `update_guild_subscriptions(Map)`; `request_members` takes `MemberRequest`/`MemberLookup`. 07's burst test now uses `RequestMembers`.
- Mockcord: op 8 rate-limit control; list DELETE when a listed member is kicked/banned. 5 tests in `tests/session/members.rs`.
- For 04: confirm `member_updates:false` and `guild_id` as an array in op 8 on real traffic. For 17: call `request_members`. For 29: send `SetMemberViewport` on visible-row changes; size from `row_count()`, placeholders for `None`. For 49: roles from `guild.members`, owner from `GuildState::owner_id`; other guilds can lose role data until reopened.
- The session suite took 27–44 s per run on the merged tree, up from about 5 s, while other lanes built; see the shutdown-stall note in 12.
