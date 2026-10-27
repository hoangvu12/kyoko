# 79 — Emoji picker: click inserts, message hover stays, loads fast

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). In the emoji picker, a left click on an emoji shows the "Favorite" context menu (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_8.png`) instead of inserting the emoji. Left click inserts (Shift keeps the picker open, as in Discord); right click opens the menu with Favorite.

**Blocked by:** none

**Status:** open

- [ ] Left click inserts the emoji into the composer (or reacts, when the picker was opened from a message's reaction button); right click opens the Favorite menu
- [ ] In-process UI test (zgx `Ui::dispatch`) for both buttons
- [ ] Same check for the sticker and GIF tabs
- [ ] Opening the picker from a message's reaction button keeps that message's hover state and toolbar shown while the picker is open (Discord, `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_4.png`); today the hover disappears
- [ ] The emoji and sticker pickers open fast: today emojis take about 3 s and stickers longer. Measure first (where the time goes: building every cell, decoding every image, fetching); then only build and fetch what's visible (virtualised rows, images requested for visible cells only, static first frame until hover where Discord does that). Target: the picker shows its first rows within one frame of opening and images fill in as they arrive; numbers before and after in the Answer
