# 79 — Emoji picker: clicking an emoji opens Favorite instead of using it

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). In the emoji picker, a left click on an emoji shows the "Favorite" context menu (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_8.png`) instead of inserting the emoji. Left click inserts (Shift keeps the picker open, as in Discord); right click opens the menu with Favorite.

**Blocked by:** none

**Status:** open

- [ ] Left click inserts the emoji into the composer (or reacts, when the picker was opened from a message's reaction button); right click opens the Favorite menu
- [ ] In-process UI test (zgx `Ui::dispatch`) for both buttons
- [ ] Same check for the sticker and GIF tabs
