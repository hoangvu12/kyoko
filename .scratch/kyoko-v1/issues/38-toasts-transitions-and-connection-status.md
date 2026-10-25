# 38 — Toasts, transitions and connection status

**What to build:** Toasts, view transitions and a connection status indicator behave as in oxidcord.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** claimed

- [ ] Toasts and transitions match oxidcord in the parity captures
- [ ] The connection status shows live, reconnecting and offline states

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
