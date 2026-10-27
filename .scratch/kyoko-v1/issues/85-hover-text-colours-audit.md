# 85 — Hover states: one rule for text colour

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Many items turn their text dark on hover where Discord turns it white (or brighter). The user asks whether kyoko has a design system. It does (Midnight tokens, `docs/design/midnight.md`), so this is an audit: find every hover and selected state and make it follow one rule from the Midnight spec.

**Blocked by:** none

**Status:** open

- [ ] Inventory of hover and selected states (sidebar rows, menus, buttons, tabs, picker items, settings rows) with their current text colour
- [ ] One documented rule in `docs/ui.md` (hover and selected text colours), using Midnight tokens only
- [ ] Every listed state follows it; captures of the main screens checked by eye
