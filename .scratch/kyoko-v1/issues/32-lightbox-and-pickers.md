# 32 — Lightbox and pickers

**What to build:** The image lightbox and the emoji, GIF and sticker pickers match oxidcord.

**Blocked by:** 26 (Message rendering)

**Status:** ready-for-agent

- [ ] The lightbox and pickers match oxidcord in the parity captures
- [ ] The lightbox shows images at full resolution
- [ ] Picking an emoji, GIF or sticker inserts or sends it

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
