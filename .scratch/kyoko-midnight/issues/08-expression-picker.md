# 08 — Expression picker: GIFs, stickers and emoji

**What to build:** Rebuild the pickers as Discord's one popover with GIFs, Stickers and Emoji tabs, anchored to the composer button that opened it (measured: 498 by 486, bottom 8 above the composer, right edge 35 in from the composer's right edge, never centred). Header with tabs and search; GIFs: category tiles, Favorites, Trending, search results in a two-column masonry with the favourite star; Emoji: Top Emoji in this server, Favorites, Frequently Used, servers, Unicode categories, the category rail, the hover footer, skin tones, search; emoji kyoko cannot send there greyed with a lock and the reason (no upsell). Stickers: Frequently Used and server packs. GIF search goes through the web client's current provider (Discord's client now says "Search Klipy"; `crates/rest/src/api/expressions.rs` still sends `provider=tenor`). Favourites (GIFs and emoji) read and write the user settings proto as the web client does.

**Blocked by:** 01, 04; v1 32

**Status:** resolved

**Mockups:** `docs/design/mockups/gif-picker.html`, `docs/design/mockups/gif-picker-results.html`, `docs/design/mockups/sticker-picker.html`, `docs/design/mockups/emoji-picker.html`, `docs/design/mockups/emoji-picker-search.html`, `docs/design/mockups/emoji-picker-no-nitro.html`

**Spec:** `docs/design/midnight.md`, Expression picker

- [x] The picker matches the mockups and opens anchored to its button
- [ ] GIF search uses the provider the web client uses, checked against the live client and noted in the fingerprint docs
- [x] Favouriting a GIF or emoji persists through the settings proto, with session-seam tests
- [x] Locked emoji show why and cannot be picked

## Answer

Merged to main as `2eee428`. Docs: `docs/ui.md` ("Expression pickers", picker ids), `docs/rest.md` (top emoji, GIF routes), `docs/fingerprint.md` ("GIF provider"), `docs/parity.md`.

- One popover, 498×486, tabs GIFs / Stickers / Emoji; the reaction (28) and custom-status (30) pickers still use it. Anchored: right edge = the live button-bar box (sent with each `composer.picker` click) minus 35, top 8 above the composer, which `composer.luau` reports as a `composer` slot (`actions.report_slot("composer", element)`). No drop shadow (zgx draws it into the opaque popup window); `motion::popover` no longer registers M07's scrim (only modals dim).
- Header per spec: text tabs, 38-high search, "Search Klipy" / "Find the perfect sticker" / "Find the perfect emoji", the hovered emoji's `:name:` as placeholder; the emoji tab adds the skin-tone button and "Add Emoji" (when you may create emoji there; opens server settings on Emoji).
- Emoji: one scrolling grid (Top Emoji in <server>, Favorites, Frequently Used, each server, People … Flags), 48 cells with 40-point emoji, 28-high collapsible headers, rail with pill following the scroll, 47-high footer. Top emoji from `GET /guilds/{id}/top-emojis` (`Command::LoadTopEmojis`, once per server per launch when the picker first opens there, `State::top_emojis`). Locked emoji: greyscale 50%, a lock on the section, footer reason ("You need Nitro to use this emoji here" or unavailable/permission); never an upsell; cannot be picked.
- GIFs: Favorites, Trending GIFs (`/gifs/trending-gifs`), category tiles; a tile opens a back-arrow view with a two-column masonry; hovered GIF gets the accent border and star. `/gifs/suggest` no longer called. Stickers: Frequently Used first, servers and packs, 4 columns, same rail and footer.
- Favourite emoji: right-click a cell → Favorite/Unfavorite, through the favourite writer shared with GIFs (`favorite_writes.rs`: read settings first, write only the emoji field; unfavourite removes the entry exactly as stored). `EmojiCell` carries `lock`, `favorite_entry`, `favorite`.
- GIF provider: `GIF_PROVIDER` in `crates/rest/src/api/expressions.rs` sends no `provider` parameter, display name Klipy. Evidence: userdoccers `17502f2` (2026-01-15) added optional `provider` (tenor/giphy/klipy, default tenor); `7927788` (2026-07-15) removed it and deprecated `/gifs/suggest` and `/gifs/trending-search`; Tenor's API shut down 2026-06-30 and the picker reads "Search Klipy". Box 2 stays open until a live check confirms: any `provider=` and its value, a `limit` on `/gifs/trending-gifs`, the GIF media hosts (asked of the user, 2026-10-26).
- Tests: session tests for emoji favourites, GIF favourites and top emoji; bridge tests (favourites read back with the stored entry; `locked_emoji_say_why_and_cannot_be_picked`). Checks re-run by the orchestrator in lane C: all green.
- Mockup parity (port 8493): emoji-picker 21.7%, gif-picker 29.1%, sticker-picker 22.6%, gif-picker-results 30.8% (new), emoji-picker-search 19.5% (new), emoji-picker-no-nitro 22.5% (new; mockcord `--no-nitro`, passed by the parity tool for that screen only). By eye tabs, header, field, rail, headers, cells, footer, tiles, masonry, star and lock state line up; the composer sits ~13 points higher than the mockup's until M16.
- Deviations: no POPULAR tag or NEW ribbon (no data); no visible focus ring on the search field; picker search isn't ranked by plugins (`hook:emoji_rank` ranks the composer's suggestions); capture steps set hover state directly (the picker is its own popup window). Seen by the orchestrator: without Nitro the current server's section also shows the lock when some of its emoji are animated (the mockup locks only other servers' sections).
- For later tickets: M06: the reaction picker has no favourite handler (set `picker.on_favorite` sending `SetEmojiFavorite`); reaction-mode header is 80 high. M09: use `EmojiCell`'s `lock`/`favorite`. M16: keep `report_slot("composer")` on the input column, keep passing the button-bar box to `composer.picker` with its right edge at the composer's right edge, keep the button ids; `capture::COMPOSER_PICKER` is only a fallback.
