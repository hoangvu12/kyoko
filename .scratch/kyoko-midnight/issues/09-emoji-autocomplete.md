# 09 — Emoji autocomplete

**What to build:** The `:` autocomplete attached above the composer in Midnight's style: "EMOJI MATCHING :sm", rows with the emoji, `:name:` and the server for custom ones, locked rows greyed with a lock, the selected row washed, the keyboard hint line. The same container serves `@` and `#` suggestions.

**Blocked by:** 03, 04

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/emoji-autocomplete.html`

**Spec:** `docs/design/midnight.md`, Expression picker

- [ ] Matches the mockup
- [ ] Locked emoji are listed but cannot be inserted
- [ ] `@` and `#` suggestions share the container
