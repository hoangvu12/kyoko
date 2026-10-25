# 33 — Threads and forums

**What to build:** Threads, forum lists, posts, pins and thread creation match oxidcord.

**Blocked by:** 26 (Message rendering)

**Status:** resolved

- [x] Thread and forum screens match oxidcord in the parity captures
- [x] Opening, following and creating threads and posts works
- [x] The pins list works

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `0acd654..4ab86c5`. Docs: `docs/ui.md`, `docs/rest.md`, `docs/parity.md`.

- Session and REST: pins via the web client's `GET /channels/{id}/messages/pins` and `PUT`/`DELETE /channels/{id}/messages/pins/{m}`; archived threads and forum posts via `GET /channels/{id}/threads/search` (a forum loads in one page with each post's first message, no request per post). `GET /guilds/{id}/threads/active` is no longer called (not for user accounts); active threads come from the gateway; `LoadActiveThreads` replaced by `LoadForumPosts`. A forum post goes the send path: lock, Send Messages check, upload limits, `before_send` hook, upload slots, then `POST /channels/{forum}/threads` with `applied_tags` and `message.attachments`. Thread and post creation are paced as message sends; joining a thread is held by the safety lock; selecting a forum loads no history. No `location` field on thread requests (web client values unconfirmed). Thread member lists are not fetched (panel shows `member_count`, as oxidcord). While a thread is open, the session treats it as selected, so the parent channel isn't auto-acked (as oxidcord).
- Window: threads open in a 420 pt panel beside their channel with their own timeline and composer (the composer host code is now shared by both); forums replace the timeline with a forum view (sort, list and gallery layouts, tag filter, paging at the end; gallery cards show a placeholder, no thumbnails). Threads popout, pins popout and the Create Thread / Create Post dialogs are drawn in Rust above the slot. Message menu Create Thread, thread chips and "started a thread" rows open the thread; the quick switcher lists forums; search hits in threads open in the panel. The thread panel's GIF, sticker and emoji buttons do nothing yet.
- Tests: 10 session-seam tests in `tests/session/threads.rs` (open, follow, create threads and posts, pins load and reload, pacing; the pacing test was shown to fail without the rule), 3 bridge tests. Created a thread and a tagged forum post in the running app. Luau button wiring checked with a throwaway headless test because of the click regression filed as 61. Checks re-run by the orchestrator in lane C.
- Parity (port 8493, 150% scale): threads-list 6.2%, thread-panel 6.6%, thread-create 7.1%, pins 6.3%, forum 4.5%, forum-post 4.9%, forum-post-create 5.2%. Remaining: text rendering, avatar art, earlier ellipsis, dimmer placeholder, dialog fields 2–3 pt lower.
- Mockcord: forum fixture (its first-message id doesn't equal the post id); the old pins and active-threads routes remain for oxidcord.
- For 35: Forward added to the message actions also works in the thread panel. For 36: `bridge::ThreadHeader.can_manage` drives Archive/Unarchive; lock, delete and rename belong in the thread panel header; channel-opening actions go in `needs_open_channel` (which now includes Create Thread). For 45/55: channel routing goes through `Link::route_channel` and `ChannelKind::opens()`.
