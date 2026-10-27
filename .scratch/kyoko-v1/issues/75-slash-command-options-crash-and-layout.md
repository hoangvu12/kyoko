# 75 — Slash command options: crash, caret gap, options not shown

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Typing a slash command (`/dl` from a bot, two optional options) shows the caret far to the right of the command name (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_9.png`), the options list only appears after clicking "+2 optional" (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_10.png`), and **picking an option crashed kyoko**. Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_11.png`) shows the options list at once when a command with options is chosen, and puts a dim "+2 options" hint right after the caret inside the composer.

**Blocked by:** none

**Status:** open

- [ ] Root cause of the crash found and fixed; a test that picks an optional option of a command (mockcord command with only optional options, string type) and fails without the fix
- [ ] The caret sits right after `/dl ` (no gap); the composer's text layout is correct with the command chip
- [ ] Choosing a command that has options opens the options list right away (required first, then optional), as Discord does; the "+N options" hint follows the caret
- [ ] Picking an option inserts its chip and moves the caret into its value; Esc and the close button still work
- [ ] Parity against the Midnight command-picker mockups where one exists; kyoko-only capture otherwise
