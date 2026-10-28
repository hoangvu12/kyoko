# 89 — Image viewer like Discord's

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Clicking an image in Discord opens a viewer (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_3.png`): the image large and centred over a dark scrim, the author and time at the top left, and a toolbar at the top right with zoom, forward, download (save), open in browser, more (copy image, copy link), and close; zoom with the wheel and drag to pan; arrows between the message's images. Copy that UI.

**Blocked by:** none

**Status:** claimed

- [ ] Viewer opens from attachments and embed images, full window over a scrim, Esc and click outside close
- [ ] Toolbar: zoom in/out (and wheel zoom, drag to pan, double-click to reset), forward (opens kyoko's forward dialog), save (Windows save dialog, original file), open in browser, copy image, copy link, close; left/right between images of the same message
- [ ] Author avatar, name and time at the top left
- [ ] Capture checked by eye; in-process UI test for open, zoom, next and close
