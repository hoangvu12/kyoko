# 26 — Drop the member search field

**What to build:** Remove v1 29's "Search members" field from the top of the member list. Discord and `midnight-discord.html` have none (user decision, 2026-10-26: the design wins where it differs from a v1 behaviour). Remove the field, its filter state and actions, its load-check sample and capture steps; keep member list scrolling and op 8 subscriptions as they are.

**Blocked by:** 03

**Status:** claimed

**Mockups:** `docs/design/mockups/midnight-discord.html`

- [ ] The member list starts with its first group header, as in the mockup capture (02)
- [ ] No dead filter code or shared values are left behind; the load check and `ui::root` pass
