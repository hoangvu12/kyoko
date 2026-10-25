# 36 — Moderation and server settings

**What to build:** The moderation dialogs and server settings pages match oxidcord: overview, roles, members, channels, emoji, stickers, invites, bans and audit log.

**Blocked by:** 23 (Channel sidebar)

**Status:** claimed

- [ ] Moderation and server settings screens match oxidcord in the parity captures
- [ ] Edits are permission-checked and paced
- [ ] The audit log and bans list load

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
