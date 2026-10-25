# 25 — Timeline view

**What to build:** The message timeline is a Rust view next to the Luau UI. It follows new messages while the user is at the bottom, keeps its place when older history loads above, and loads earlier pages as the user scrolls up.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** claimed

- [ ] The timeline uses measured rows keyed by message id
- [ ] It stays at the end while the user is at the bottom
- [ ] When older history is added above, the first visible row stays in place
- [ ] Scrolling up loads older pages automatically
- [ ] Plain text messages render in oxidcord's grouped layout and match the parity captures

## Comments

From the spike (04): replace the spike's `bridge::Timeline` (whole-list projection, plain text) with the Rust timeline view; publish changes instead of the whole list; show local timestamps. Posted messages reached the UI in a median of 1.4 ms with none dropped or duplicated. Ack behaviour belongs to the session (ticket 10): send `SetViewport` and `SetViewing`, never per-message acks.
