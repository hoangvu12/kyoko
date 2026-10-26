# 06 — Reactions, super reactions and the reactions list

**What to build:** Reaction chips in Midnight's tokens (24 high, 2-point borders, reacted `--accent-2`), super reactions (burst colours as the chip's gradient border, a sparkle after the count, burst-colour fill when it is yours), the hover tooltip ("hungry, edelweiß and 2 others reacted with :thumbsup:"), the add-reaction and add-super-reaction buttons after the last chip, and the Reactions list dialog (Reactions and Super Reactions tabs, reaction column, who reacted, remove for moderators). Adding a super reaction is offered only to accounts that can send one; kyoko never shows an upsell.

**Blocked by:** 05

**Status:** claimed

**Mockups:** `docs/design/mockups/reactions.html`, `docs/design/mockups/reactions-list.html`

**Spec:** `docs/design/midnight.md`, Palette, Expressions and commands

- [ ] Chips, tooltip and list match the mockups
- [ ] Super reactions are read from `burst`, `me_burst` and `burst_colors` and toggle with the burst type
- [ ] The reactions list loads who reacted, paged, and removing one is gated on Manage Messages
- [ ] Session-seam tests cover super reactions and the list against mockcord
