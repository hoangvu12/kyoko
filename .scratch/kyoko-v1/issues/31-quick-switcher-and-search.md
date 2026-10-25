# 31 — Quick switcher and search

**What to build:** The quick switcher and message search match oxidcord. Search results offer the same full context menu as the timeline, which is the built-in version of FullSearchContext.

**Blocked by:** 28 (Message actions and context menus)

**Status:** resolved

- [x] The quick switcher and search panel match oxidcord in the parity captures
- [x] Jumping to any server, channel or DM by name works
- [x] Search with filters works, and results open in context
- [x] Search results use the timeline's full context menu

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `b475db6..92a08c8`. Docs: `docs/ui.md`, `docs/parity.md`, `docs/rest.md`.

- Session: message search (`GET /guilds/{id}/messages/search`, DMs on `/channels/{id}/messages/search`) with `from:`, `mentions:`, `has:`, `before:`, `after:`, `during:`, `in:`, `pinned:` filters and offset paging. Paced: 300 ms of quiet (`RestTuning::search_delay`), one request in flight, stale replies dropped; a 202 "index not ready" is retried after its `retry_after`, giving up after 5 tries; a refused search (403) reports why and isn't retried. `RestClient::execute_with_status` added in `rest` to tell a 202 apart. `State::detached_view` added. 7 seam tests in `tests/session/search.rs`; mockcord routes and fixtures.
- Bridge: `bridge/switcher.rs` ranks places as oxidcord (fuzzy score, category grouping, recency tie-break, `#`/`!`/`@`/`*` prefixes, 9-result cap, recent places); search parsing and hit lowering reuse 26's `MessageEntry`. 10 bridge tests.
- Window: switcher and search panel drawn in Rust as a layer in the root `overlay()` after 30's profile layer. Ctrl+K (bubble phase, after 30's Escape handler) or the DM sidebar's "Find or start a conversation" opens the switcher (`dms.search` no longer a stub); Enter in the top bar's search field runs a search, filters shown as chips; the results panel loads the next page when the last card comes into view (oxidcord shows only the first page). Picking a server only selects it, like the rail. Hits open in context via `TimelineView::jump_then(message_id, action)`; hits without `guild_id` (threads) take their server from the search scope.
- FullSearchContext: right-click on a hit opens the timeline's own menu (`bridge::message_menu` via `MessageActions::menu_items(entry, access)`, with the hit's channel permissions); actions needing the channel open (React, Reply, Edit, Select; list `needs_open_channel` in `timeline/actions.rs`) open the hit first, then run.
- Shared additions: theme token `focus_ring` (`theme.rs`, `theme.luau`); 7 channel-kind icons in `timeline/icons.rs`; search panel in 60's load check.
- Parity (port 8493, 150% scale): quick-switcher 6.5%, search 6.4%; layout matches row for row (including oxidcord's 360 pt list cap). Remaining: focused caret, dimmer placeholder, avatar art, a scrollbar-gutter difference in result snippets. Checks re-run by the orchestrator in lane C.
- For 33: the switcher lists active threads; forums are left out until the forum view exists (`opens` in `bridge/switcher.rs`); Create Thread from a hit uses the same open-in-context path. For 36: anything added to `bridge::message_menu` appears on search hits too; add channel-open actions to `needs_open_channel`. For 45/53: plugin menu items go into `bridge::message_menu` and `MessageActions::menu_items(entry, access)`, which draws the timeline menu, the hover bar's More menu and the search menu; use the passed `access`, not `self.access`.
