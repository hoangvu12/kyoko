# 30 — User panel, status and profiles

**What to build:** The user panel, the status and custom status controls, the profile popout and the full profile all match oxidcord.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** ready-for-agent

- [ ] The user panel, popout and full profile match oxidcord in the parity captures
- [ ] Changing status and custom status works
- [ ] Presence and activities show on profiles

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
