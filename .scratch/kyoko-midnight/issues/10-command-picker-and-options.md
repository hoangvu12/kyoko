# 10 — Slash command picker and options

**What to build:** The `/` picker as Discord's: a panel 8 above the composer and as wide as it, the app rail, sections (Frequently Used, each app, Built-In last), rows 56 apart with option chips. Picking a command turns the composer into the app icon, `/name` and one chip per option, with the hint bar above (option name and description, clear button, "+N optional" opening the optional list). Values show in their chips; an invalid value turns its chip and the hint red with the reason. Only the emoji button stays at the composer's right while a command is open.

**Blocked by:** 03; v1 27

**Status:** claimed

**Mockups:** `docs/design/mockups/command-picker.html`, `docs/design/mockups/command-options.html`, `docs/design/mockups/command-options-optional.html`, `docs/design/mockups/command-options-remind.html`, `docs/design/mockups/command-choices-invalid.html`

**Spec:** `docs/design/midnight.md`, Command picker and options

- [ ] The picker and option states match the mockups
- [ ] Required options are checked before sending; the reason shows in the hint bar
- [ ] Frequently Used is kept per account
- [ ] Bridge tests cover option parsing and validation
