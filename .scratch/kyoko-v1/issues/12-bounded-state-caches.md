# 12 — Bounded state caches

**What to build:** kyoko's memory stays bounded no matter how long it runs or how many channels are opened.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** resolved

- [x] Messages per channel are capped. A limited set of warm channels keep message bodies, and colder channels keep metadata only
- [x] Profile and member caches are capped
- [x] Reading back through history widens a channel's window temporarily, then shrinks it
- [x] Session tests assert the caps after opening many channels and long histories

## Answer

Merged to main as `512b5b3` (rebased over 09 by the orchestrator; conflicts were both sides adding `SessionConfig` fields and imports). Docs: `docs/caches.md`.

- `state::CacheLimits` via `SessionConfig::caches`: `messages_per_channel` 150, `history_ceiling` 1000, `warm_channels` 8, `members` 5000 (total), `users` 10,000, `profiles` 128. Over a limit, trim to 7/8 of it. `MESSAGE_CAP` is gone.
- Warm set: a channel warms when selected, when history loads, or on send; least recently used goes cold beyond 8 (never the open channel or one with pending sends). Cold channels keep last message id, read state and mention counts; reopening reloads from sqlite then REST.
- Window: older pages widen the open channel up to the ceiling; past it the newest are dropped, the window is detached, and `LoadNewer` refetches. It shrinks back on returning to the bottom (not detached) or leaving.
- Viewport: `Command::SetViewport { channel_id, viewport: Viewport { first, last, at_bottom } }`; trimming never drops rows between `first` and `last`. `ChannelMessages::resident/is_resident/viewport/window/is_detached`. Viewport changes publish nothing unless the window shrinks.
- People: `state::Recent` LRU; never evicted: current user, authors in warm channels, typers, voice-state users, open guild's member-list rows; for users also friends, DM recipients, anyone with a member entry. Memory only; no store rows deleted.
- First page into an empty channel now trims its oldest messages, not its newest.
- Still unbounded (out of scope): plugin-attribution maps, pins, reaction-user lists, invite previews, audit logs, bans. Evicting a member from an unopened guild can show rows without role colours until the guild reopens.
- Tests: 5 in `tests/session/caches.rs`. One combined-tree suite run took 66 s (usual 7 s): the session-shutdown stall that 06 also saw; worth investigating.
- For 16: keep deleted/edited versions in the store; anything in memory hangs off `ChannelMessages`; eviction calls no delete paths. For 25: send `SetViewport` when the first/last visible row changes and at the bottom; key rows by message id; use `is_detached()` and `LoadNewer`.
