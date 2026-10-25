# 28 — Message actions and context menus

**What to build:** Every message action oxidcord offers is available from the context menu and keyboard shortcuts.

**Blocked by:** 26 (Message rendering), 10 (Paced read receipts and safe multi-delete)

**Status:** claimed

- [ ] The context menu matches oxidcord in the parity captures
- [ ] Copy text, copy link, copy ID, reply, edit, delete, pin, mark unread and react all work
- [ ] Multi-select delete uses the safe multi-delete path
- [ ] The emoji reaction picker works

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
