# 91 — Videos play inline (embeds and attachments)

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Video embeds (for example a TikTok link through a fixer, `https://vt.tiktokez.com/ZS9bbCDus/`) show a thumbnail with a play button that does nothing in kyoko (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_5.png`); Discord plays them inline with controls (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_6.png`: play/pause, time, seek bar, volume, fullscreen). Same for video attachments.

**Blocked by:** none

**Status:** open

- [ ] Research and pick a video path for a native Windows app (Media Foundation via the `windows` crate is the first candidate: no extra DLLs; compare with alternatives on size, codecs (H.264, VP9, AV1 where the OS has them), licence); write the choice in `docs/media.md`
- [ ] Inline player for embed videos and video attachments: click to play, controls as in Discord (play/pause, time, seek, volume/mute, fullscreen), muted autoplay only where Discord does it (GIFV), pause when scrolled out of view
- [ ] Video fetched through the media client (Chrome handshake, allow-list, size caps; streamed, not loaded whole into memory)
- [ ] Bridge or desktop tests with a small mock mp4; capture checked by eye
