# 87 — GIFs and animated stickers play

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). GIFs (attachments, GIF embeds from Tenor/Klipy, which are often `.mp4`/`.webm` on real Discord) and animated stickers (APNG, Lottie JSON, GIF stickers) don't play.

**Blocked by:** none

**Status:** resolved

- [x] Find why animation doesn't play on real data (format mismatch such as mp4 GIFs, playback setting, visibility, decoder); fix
- [x] GIF attachments and GIF embeds animate (mp4/webm GIFs need a video path: if 91's video decoder isn't there yet, use the `.gif` or animated `.webp` the media proxy offers via `format=`), respecting the playback setting and reduced motion
- [x] Animated stickers: GIF and APNG stickers animate; Lottie stickers at least show their first frame (or animate if a small renderer fits)
- [x] Bridge seam tests with mock animated GIF, WebP, APNG and a GIF embed; capture checked by eye

## Answer

Merged to main as `e1684a4`, `1b4eed9`, `0af11d1` (worker `9dc9e7f0`, lane C; rebased by the orchestrator onto `8c0321d`, clean; checks re-run: all green; parity timeline-files 16.2%, timeline-attachments 19.2%, checked by eye). Docs: `docs/media.md`, `docs/ui.md`, `docs/parity.md`.

- Root causes: the 1/8-of-budget animation share (16 MiB) cut most real GIFs to their first frame; `gifv` embeds only requested the still thumbnail; animated WebP embeds lacked `animated=true`; Lottie stickers were never requested; APNG/GIF stickers hit the same share.
- Animations over the share are now streamed (`Decoded::Streamed`, a `media-stream` thread decodes in order while visible; memory counted as first frame x4 plus file size). Frames are shrunk and masked while decoding (`media::Plan`).
- GIF embed URL, first that applies: thumbnail/image already GIF or flagged animated (`flags` bit 5); Tenor video `media.tenor.com/<id><code>/<name>.mp4` becomes `media.tenor.com/<id>AAAAC/<name>.gif` (naming rule from memory, untested live); else the proxy `...x.mp4?format=webp&quality=lossless&animated=true&width=W&height=H`. Every GIF embed request falls back to the still thumbnail (`ImageRequest::or_else`).
- Stickers: APNG `cdn.discordapp.com/stickers/<id>.png?size=320&passthrough=true`, GIF `media.discordapp.net/stickers/<id>.gif?size=320`, Lottie `cdn.discordapp.com/stickers/<id>.json` drawn by `crates/media/src/lottie.rs` (tiny-skia; no masks, mattes, trim paths, repeaters, stars, text, images). Stickers report hover for on-hover playback.
- Mockcord `/media/<hash>.<ext>` serves 8-frame GIF, WebP, APNG and Lottie (`MEDIA_FRAMES`); kyoko's capture pages back to find `timeline-embeds-media` (oxidcord's steps still can't reach it; needs an oxidcord-ref patch).
- Open on real Discord: whether the proxy animates mp4-only GIFs (Klipy etc.) as WebP, stays still, or refuses (415); the Tenor `AAAAC` rule.
- For 91: replace the mp4 stand-in in `bridge::gifv_request`; the `media-stream` thread is a model for video frames.
