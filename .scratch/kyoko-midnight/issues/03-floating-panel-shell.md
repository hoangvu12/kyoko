# 03 — Floating-panel shell

**What to build:** Restyle the shell to Midnight's floating panels: the `--bg-3` frame, every region a `--bg-4` panel with `--radius-lg` corners and a 1-point `--active` border, 12-point gaps and insets, the 12-point title strip with the three coloured window-control circles at the top right, the server rail (66 wide, 40-point rounded-square icons, folders, the 4-point pill), the channel sidebar (280, rows 36 high and 38 apart, the sliding hover and the unread dot), the user panel (its own panel under the sidebar, small-user-panel mode), the channel header (50 high, inbox and help at its right end), the chat panel (timeline and composer together) and the member list (264, rows 44 apart, sliding hover). Motion stays behind reduced motion.

**Blocked by:** 01; v1 20, 22, 23, 24, 29, 30

**Status:** claimed

**Mockups:** `docs/design/mockups/midnight-discord.html`, `docs/design/mockups/dm.html`, `docs/design/mockups/group-dm.html`

**Spec:** `docs/design/midnight.md`, Structure: floating panels, Server rail, Channel sidebar, User panel, Channel header, Member list, Motion

- [ ] The main screen matches `midnight-discord.html` in a mockup capture (02)
- [ ] The measured sizes in the spec hold at 100% scale
- [ ] Hover slides and the home icon spin respect reduced motion
- [ ] The headless load check builds every screen with the new shell
