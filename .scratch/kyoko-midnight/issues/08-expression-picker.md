# 08 — Expression picker: GIFs, stickers and emoji

**What to build:** Rebuild the pickers as Discord's one popover with GIFs, Stickers and Emoji tabs, anchored to the composer button that opened it (measured: 498 by 486, bottom 8 above the composer, right edge 35 in from the composer's right edge, never centred). Header with tabs and search; GIFs: category tiles, Favorites, Trending, search results in a two-column masonry with the favourite star; Emoji: Top Emoji in this server, Favorites, Frequently Used, servers, Unicode categories, the category rail, the hover footer, skin tones, search; emoji kyoko cannot send there greyed with a lock and the reason (no upsell). Stickers: Frequently Used and server packs. GIF search goes through the web client's current provider (Discord's client now says "Search Klipy"; `crates/rest/src/api/expressions.rs` still sends `provider=tenor`). Favourites (GIFs and emoji) read and write the user settings proto as the web client does.

**Blocked by:** 01, 04; v1 32

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/gif-picker.html`, `docs/design/mockups/gif-picker-results.html`, `docs/design/mockups/sticker-picker.html`, `docs/design/mockups/emoji-picker.html`, `docs/design/mockups/emoji-picker-search.html`, `docs/design/mockups/emoji-picker-no-nitro.html`

**Spec:** `docs/design/midnight.md`, Expression picker

- [ ] The picker matches the mockups and opens anchored to its button
- [ ] GIF search uses the provider the web client uses, checked against the live client and noted in the fingerprint docs
- [ ] Favouriting a GIF or emoji persists through the settings proto, with session-seam tests
- [ ] Locked emoji show why and cannot be picked
