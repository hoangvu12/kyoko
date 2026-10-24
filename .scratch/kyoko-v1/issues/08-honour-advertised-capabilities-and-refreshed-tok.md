# 08 — Honour advertised capabilities and refreshed tokens

**What to build:** kyoko only asks Discord for behaviours it handles, and keeps the stored token current when Discord refreshes it.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** ready-for-agent

- [ ] The advertised capability set contains only bits whose effects are implemented, guided by the spike findings
- [ ] A refreshed token delivered on connect replaces the stored one
- [ ] If batched reactions or passive guild updates are advertised, their dispatches are parsed and applied
- [ ] Session tests cover the refreshed token and the effect of each advertised capability
