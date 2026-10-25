# 31 — Quick switcher and search

**What to build:** The quick switcher and message search match oxidcord. Search results offer the same full context menu as the timeline, which is the built-in version of FullSearchContext.

**Blocked by:** 28 (Message actions and context menus)

**Status:** ready-for-agent

- [ ] The quick switcher and search panel match oxidcord in the parity captures
- [ ] Jumping to any server, channel or DM by name works
- [ ] Search with filters works, and results open in context
- [ ] Search results use the timeline's full context menu

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
