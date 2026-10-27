# 79 — Emoji picker: click inserts, message hover stays, loads fast

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). In the emoji picker, a left click on an emoji shows the "Favorite" context menu (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_8.png`) instead of inserting the emoji. Left click inserts (Shift keeps the picker open, as in Discord); right click opens the menu with Favorite.

**Blocked by:** none

**Status:** resolved

- [x] Left click inserts the emoji into the composer (or reacts, when the picker was opened from a message's reaction button); right click opens the Favorite menu
- [x] In-process UI test (zgx `Ui::dispatch`) for both buttons
- [x] Same check for the sticker and GIF tabs
- [x] Opening the picker from a message's reaction button keeps that message's hover state and toolbar shown while the picker is open (Discord, `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_4.png`); today the hover disappears
- [x] The emoji and sticker pickers open fast: today emojis take about 3 s and stickers longer. Measure first (where the time goes: building every cell, decoding every image, fetching); then only build and fetch what's visible (virtualised rows, images requested for visible cells only, static first frame until hover where Discord does that). Target: the picker shows its first rows within one frame of opening and images fill in as they arrive; numbers before and after in the Answer

## Answer

Merged to main as `eec19a5`, `7426ef7` (worker `474a9e38`, lane C; checks re-run by the orchestrator: all green; parity emoji-picker 20.7%, sticker-picker 22.4%, at or below Midnight 08's). Docs: `docs/ui.md` (pickers, hover bar), `docs/testing.md` (mockcord flags).

- Left click: zgx's `menu` treats an element with its own handlers as a menu button (a left press opens the menu and cancels the click). The Favorite menu now has its own empty anchor behind the cell and is built on the first right click (`ExpressionPicker::expression_menu`). Left click picks (composer or reaction), Shift keeps the picker open, right click offers Favorite/Unfavorite plus plugin items. GIF tiles got the same right-click Favorite; stickers have none.
- Hover: a message counts as hovered while the reaction picker opened from it is open (`MessageActions::holds_hover`, effect in `TimelineView::row`).
- Speed: building was never slow (first frame 20–40 ms); the time went on fetching in the wrong order (the emoji rail fetched all server icons first; stickers fetched all 442 at once). Grids and rails are now virtualised, only visible rows' images are requested, custom emoji stay still until hovered, loaded images are kept across opens. Mockcord crowded mode (50 servers, 3150 emoji, 412 stickers, 120 ms per image): emoji first rows 1.66–1.89 s -> 1.29–1.58 s (107 -> 69 fetches); stickers 1.1–10.0 s -> 0.27–0.50 s (442 -> 17); second open 2–15 ms. The remaining emoji time is the media pipeline's 6-at-a-time cap and unordered disk checks: ticket 93.
- Tests: `pickers::tests::a_left_click_on_an_emoji_picks_it_and_a_right_click_offers_favorite`, `a_left_click_on_a_sticker_sends_it_and_a_right_click_does_not`, `a_left_click_on_a_gif_sends_it_and_a_right_click_offers_favorite`, `timeline::react_tests::the_reaction_picker_keeps_its_message_hovered_and_a_left_click_reacts`; each fails on the old behaviour.
- Mockcord: `MockConfig::crowded()` / `--crowded-expressions`, `media_delay` per image. zgx rule: never attach a `menu` to an element with its own event handlers.

