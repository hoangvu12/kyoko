# 82 — Multiple image attachments: Discord's grid

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). A message with two images of different shapes renders them cropped side by side in a short strip (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_1.png`); Discord shows the same message as a mosaic that keeps both images readable (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_2.png`). Match Discord's attachment grid for 1 to 10 images (sizes, crops, gaps, radius).

**Blocked by:** none

**Status:** open

- [ ] Layout rules for 1 to 10 images written in `docs/` (from observation and public sources)
- [ ] The timeline lays out multi-image messages that way; single images keep their aspect ratio within the max box
- [ ] Mock data with 2, 3, 4 and mixed-shape images; capture checked by eye against the Midnight mockup if one exists
