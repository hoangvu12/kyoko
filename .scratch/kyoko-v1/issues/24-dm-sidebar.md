# 24 — DM sidebar

**What to build:** The DM sidebar matches oxidcord and orders conversations by recent activity.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] The DM sidebar matches oxidcord in the parity captures
- [x] Conversations reorder live when new messages arrive
- [x] Opening a DM works

## Answer

Merged to main as `527baf1..526081b`. Docs: `docs/ui.md`, `docs/parity.md`.

- Bridge `DmList` (`crates/bridge/src/dm_list.rs`) on `dm_rows()`: name, recipient, status (presence, offline when missing), member count, unread, mentions, `activity`; newest first by last message id (channel id when empty, as the web client); message requests and spam (`bridge::is_request`) are excluded and counted in `requests()`; unnamed groups use recipients' names; publishes only on change.
- Host `apps/desktop/src/dm_sidebar.rs` (`DmSidebarView`) publishes `dms.rows`, `dms.requests`, `home.page`; Link side in `apps/desktop/src/ui/home.rs`. Luau `shell/dm_sidebar.luau`, `shell/home_page.luau` (placeholder), reusable components `avatar.luau` and `status_dot.luau` (for 29, 30).
- Home: `bridge::Sidebar::show_home()`/`is_home()`; `sidebar.guild_id == ""` swaps `channel_sidebar`'s body; `rail.select_home` wired; Home reopens the last DM opened there or the most recent; `SelectChannel { guild_id: None }`. DM top bar: @ icon, name, status dot, call/video/pins/search/inbox/help; member list hidden. Friends/Nitro/Shop/Message Requests open the placeholder page and collapse the timeline slot (`show_home_page`), restored when a conversation or server opens.
- Close DM: hover x → confirmation popover inside the sidebar (oxidcord wording; "Leave Group" for groups) → existing `Command::CloseDm` (`DELETE /channels/{id}`). Unconfirmed: whether the web client adds a query string (e.g. `?silent=false`).
- Tests: 6 bridge tests (`crates/bridge/tests/bridge/dms.rs`), 2 session-seam tests (`tests/session/dms.rs`). Parity: dm-sidebar 4.3%, dm-group 4.8% (oxidcord's group member panel not built), channel-sidebar 7.5%; sidebar region ~3.3–3.5%.
- Gaps: the composer placeholder still says "Message #name" in DMs (27). Stubs `dms.search` (31), `dms.create_group`. Nothing calls `OpenDm` yet; whoever adds "start a conversation" must check `dm_lock(user_id)` first.
- For 34: fill `home_page.luau` driven by `home.page` (`friends`/`nitro`/`shop`/`requests`); the requests page lists `is_request` channels and applies 11's `spam_dm`/`message_request` locks. 41: reset the DM sidebar view on switch (pattern in `ui/sign_in.rs`). 51: DM mentions come from the state's `bump_mentions`.
