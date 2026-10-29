# 131 — Profile theme colours match Discord

**What to build:** From the user's real run 4 (2026-10-30), image-10 (kyoko) vs image-11 (Discord) in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy). The same user's profile theme (Nitro `theme_colors`, 92) looks darker and flatter in kyoko than in Discord. Find how the web client builds the profile gradient and overlay from the two theme colours (gradient direction and stops, the dark/light overlay alpha, how text and inner panels are tinted) and match it in the popout and the full profile.

**Blocked by:** 130 (same profile files)

**Status:** ready-for-agent

- [ ] The web client's formula written down in `docs/` with its source
- [ ] Popout and full profile match image-11 for the same colours (mockcord gets a user with those theme colours)
- [ ] Unit or bridge test of the colour maths; captures checked by eye against image-11
