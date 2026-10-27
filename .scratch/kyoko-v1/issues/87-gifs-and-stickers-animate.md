# 87 — GIFs and animated stickers play

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). GIFs (attachments, GIF embeds from Tenor/Klipy, which are often `.mp4`/`.webm` on real Discord) and animated stickers (APNG, Lottie JSON, GIF stickers) don't play.

**Blocked by:** none

**Status:** open

- [ ] Find why animation doesn't play on real data (format mismatch such as mp4 GIFs, playback setting, visibility, decoder); fix
- [ ] GIF attachments and GIF embeds animate (mp4/webm GIFs need a video path: if 91's video decoder isn't there yet, use the `.gif` or animated `.webp` the media proxy offers via `format=`), respecting the playback setting and reduced motion
- [ ] Animated stickers: GIF and APNG stickers animate; Lottie stickers at least show their first frame (or animate if a small renderer fits)
- [ ] Bridge seam tests with mock animated GIF, WebP, APNG and a GIF embed; capture checked by eye
