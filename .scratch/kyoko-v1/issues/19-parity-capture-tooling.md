# 19 — Parity capture tooling

**What to build:** A contributor can capture every parity screen from oxidcord and kyoko against the same mockcord seed, and compare them side by side.

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** ready-for-agent

- [ ] A pinned oxidcord reference build (0.2.0-beta.1) runs from a separate checkout outside kyoko's tree
- [ ] Both apps can be driven to a named screen and state on the same mockcord seed
- [ ] One command captures both sets of screenshots for every screen in the parity list
- [ ] The output pairs the two captures for each screen, for review
