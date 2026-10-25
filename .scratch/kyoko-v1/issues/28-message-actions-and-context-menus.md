# 28 — Message actions and context menus

**What to build:** Every message action oxidcord offers is available from the context menu and keyboard shortcuts.

**Blocked by:** 26 (Message rendering), 10 (Paced read receipts and safe multi-delete)

**Status:** resolved

- [x] The context menu matches oxidcord in the parity captures
- [x] Copy text, copy link, copy ID, reply, edit, delete, pin, mark unread and react all work
- [x] Multi-select delete uses the safe multi-delete path
- [x] The emoji reaction picker works
## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `c6ca33a..8843804`. Docs: `docs/ui.md`, `docs/parity.md`.

- No new Commands (Pin/Unpin, AddReaction/RemoveReaction, DeleteMessages, MarkUnread existed). New session `task/reactions.rs`: a new reaction needs Read Message History + Add Reactions, joining an existing one only history; refusals never reach REST; REST failures come back as `CommandRejected` with the reason, never retried.
- Bridge: `bridge/actions.rs` (menu items in oxidcord's order gated by channel `Access`, message links, hover-bar buttons, reaction toggles, delete-progress summary from `State::deletions()`); `bridge/emoji_picker.rs` (picker data: rail, sections, search, skin tones).
- All drawn in Rust inside the timeline view (overlay rule met): context menu is a zgx `menu` at the pointer, kept inside the window (not zgx `context_menu`, which flips); hover bar on the hovered row; delete/bulk delete/pin confirmations are zgx modals; emoji picker (`timeline/picker.rs`, `ReactionPicker`, takes a pick callback) is a popover; the selection bar floats at the bottom of the timeline and shows delete progress, lock and failure notices. Theme token `message_selected`.
- Composer hand-off: Reply/Edit call 27's `Link::composer_reply`/`composer_edit` (now `pub(super)` in `ui/composer.rs`).
- Shortcuts: Shift on Delete skips the confirmation; Shift-click selects (then plain clicks toggle); Alt-click marks unread; Escape clears selection; Enter in picker search picks the first result. Copy ID only in developer mode. Select Message offered for own messages too; Reply/Add Reaction hidden where not allowed. Forward and Create Thread are logging stubs.
- Parity tooling: `tools/xtask/src/parity/window.rs` now composites the captured process's other visible windows (zgx popups) onto the capture. Screen names `message-context-menu`, `message-hover`. Parity: message-context-menu 7.8%, message-hover 7.5%, timeline 7.6% (menu region items line up within 1 px).
- Tests: 4 session-seam (`tests/session/actions.rs`), 4 bridge (`tests/bridge/actions.rs`). Manual run driven in-process via zgx `Ui::dispatch` (copy, reply hand-off, pin, mark unread, react via picker and chip, jump to reply, multi-delete 3 of 3).
- For 30: avatar/author/mention clicks call `TimelineView::on_profile` (stub in `ui/message_actions.rs`). 31: `TimelineView::jump_to(message_id)` (scrolls or `LoadAround` then scrolls). 32: reuse `timeline/picker.rs` (or move it out of `timeline/`) for the composer's pickers. 33: fill `CreateThread` in `MessageActions::run`. 35: fill `Forward`. 45/53: append plugin items to `bridge::message_menu` and render in `menu_items`.
