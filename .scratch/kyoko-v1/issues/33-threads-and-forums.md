# 33 — Threads and forums

**What to build:** Threads, forum lists, posts, pins and thread creation match oxidcord.

**Blocked by:** 26 (Message rendering)

**Status:** ready-for-agent

- [ ] Thread and forum screens match oxidcord in the parity captures
- [ ] Opening, following and creating threads and posts works
- [ ] The pins list works

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
