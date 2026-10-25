# 17 — Fetch missing replies and users

**What to build:** Replies whose original message isn't loaded, and mentions of users kyoko hasn't seen, resolve automatically through polite background fetches. This is the built-in version of ValidReply and ValidUser.

**Blocked by:** 09 (REST safety layer)

**Status:** resolved

- [x] Missing referenced messages are fetched, cached and paced
- [x] Unknown mentioned users are fetched, cached and paced
- [x] Fetch failures don't retry in a tight loop
- [x] Session tests assert the backfill traffic and the resulting state

## Answer

Merged to main as `2b5b05e`. Docs: backfill section in `docs/rest.md`.

- Endpoints: missing reply originals via `GET /channels/{c}/messages?around={m}&limit=1` (not `/messages/{id}`); unknown mentions in guilds via op 8 (`request_members`); in DMs, and for ids op 8 reports `not_found`, paced `GET /users/{id}`. Guild message authors without a member entry get op 8 (not-found ids not re-asked for 5 min).
- Queue (`task/backfill.rs`, `RestTuning`): one request at a time, `backfill_gap` 500 ms, `backfill_per_minute` 30, ≤200 waiting jobs, `backfill_cool_down` 5 min for queued/in-flight/recently failed targets. 404/403/Missing Permissions → unavailable for good; a 429 the client couldn't absorb pauses the queue for `retry_after` and retries once, a second 429 drops it for the cool-down; suspended route pauses for the breaker's `retry_in`; captcha 10 min; network/server error 5 s; 401 stops the queue.
- Scope: only the open channel is scanned (on select, page load, message create/edit), only with View Channel + Read Message History; switching channel drops other channels' queued work.
- State: `State::reply_target(&msg) -> Option<ReplyTarget::{Message, Unavailable, Loading}>`, `State::referenced_message(id) -> Option<&Referenced::{Found(Arc<Message>), Unavailable}>`, `State::can_read_history(channel)`, `backfill_needs`, `reply_reference`. Fetched originals live in their own LRU (`CacheLimits::referenced_messages` 256), never in the channel window; cold channels drop their entries; deletes mark unavailable; edits update. `LocalEvent::ReferenceFetched` / `UserFetched`.
- 7 tests in `tests/session/backfill.rs`, each mechanism verified to fail when disabled.
- Follow-up idea: lenient decoding can't tell `referenced_message: null` (deleted) from missing, costing one fetch per deleted original.
- For 26: use `reply_target`; "Original message was deleted" for `Unavailable`; `Loading` can last forever without Read Message History, so check `can_read_history`; resolved items publish `Changes::messages`; unresolved mentions show a placeholder. For 48: nicknames arrive via op 8 after a page loads; authors who left the guild fall back to global/user name.
