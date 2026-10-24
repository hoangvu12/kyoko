# 25 — Timeline view

**What to build:** The message timeline is a Rust view next to the Luau UI. It follows new messages while the user is at the bottom, keeps its place when older history loads above, and loads earlier pages as the user scrolls up.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** ready-for-agent

- [ ] The timeline uses measured rows keyed by message id
- [ ] It stays at the end while the user is at the bottom
- [ ] When older history is added above, the first visible row stays in place
- [ ] Scrolling up loads older pages automatically
- [ ] Plain text messages render in oxidcord's grouped layout and match the parity captures
