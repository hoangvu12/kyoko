# 91 — Videos play inline (embeds and attachments)

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Video embeds (for example a TikTok link through a fixer, `https://vt.tiktokez.com/ZS9bbCDus/`) show a thumbnail with a play button that does nothing in kyoko (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_5.png`); Discord plays them inline with controls (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_6.png`: play/pause, time, seek bar, volume, fullscreen). Same for video attachments.

**Blocked by:** none

**Status:** resolved

- [x] Research and pick a video path for a native Windows app (Media Foundation via the `windows` crate is the first candidate: no extra DLLs; compare with alternatives on size, codecs (H.264, VP9, AV1 where the OS has them), licence); write the choice in `docs/media.md`
- [x] Inline player for embed videos and video attachments: click to play, controls as in Discord (play/pause, time, seek, volume/mute, fullscreen), muted autoplay only where Discord does it (GIFV), pause when scrolled out of view
- [x] Video fetched through the media client (Chrome handshake, allow-list, size caps; streamed, not loaded whole into memory)
- [x] Bridge or desktop tests with a small mock mp4; capture checked by eye

Note (2026-10-28): oxidcord (`oxidcord-ref`, MIT/Apache, reusable) has a Media Foundation video decoder in `crates/voice/src/video/decode/media_foundation.rs` (plus `mf.rs`, `convert.rs`); start from it. The voice plan (`nguyenvu/.research/voice-plan/plan.md`) ports the same code for calls, so coordinate: one shared decoder.

## Answer

Merged to main as `f2c8da6`, `294dba5` and `b11b9e8` (worker `5d0cc41b`, lane C; full verify re-run by the orchestrator: all green, tls-capture 0/0 against chrome-156, kyoko-only captures `timeline-video`, `video-player`, `timeline-embeds-media`, `timeline-attachments` checked by eye). Docs: `docs/media.md` ("Inline video": the decoder choice against FFmpeg, GStreamer, libmpv, pure Rust and WebView2; the mock clip's ffmpeg command), `docs/parity.md`.

- Root cause: no video playback at all. Embeds showed a thumbnail whose play disc opened the link, attachments were file cards, GIF embeds used 87's animated-image stand-in, the only decoder was voice's H.264 one, and the media client fetched whole files up to 64 MiB with no ranges.
- New headless crate `crates/video`: the one shared decoder (moved from `crates/voice/src/video/{convert,decode,error,media_foundation,mf}.rs`, `VideoFrame` in `lib.rs`; voice re-exports `::video::{H264Decoder, Picture, VideoError, VideoFrame, available, fit}`), plus `FileReader`/`ByteSource`: a Media Foundation source reader over a Rust `IMFByteStream` giving NV12 frames, float audio and seeking. Decodes H.264+AAC MP4, VP9+Opus WebM and AV1 MP4 here. `examples/read_clip` reports a machine's codecs.
- Media: `VideoFile`, `MediaFetcher::open_range`: Range requests through the media client (allow-list, per-host queue, Chrome handshake) into a self-deleting scratch file, at most 8 MiB ahead of the reader, restarts on seek, refuses over 512 MiB. `Fingerprint::video_headers` from the new `chrome-156-video.json` capture: the `<img>` order with `range: bytes=0-` before `priority`, `accept: */*`, `accept-encoding: identity`, weight 147; `range` added to `IMAGE_ORDER`.
- Audio `Feed`, `Deck::set_feed`: the sound track drives the clock. Bridge `VideoPlayer` (one thread per clip, frames converted at tile size, at most 30 fps, one clip with sound at a time), `VideoClip`, `mosaic_media`, `video_poster_request`, `local_video`, `Gallery::gif_playback`.
- UI: video attachments are mosaic tiles; streamable embeds play where their large image was; GIF embeds play muted and looping over their still (following GIF autoplay and reduced motion); any video pauses off screen; controls are play/pause, time, seek, mute plus volume on hover, fullscreen. Fullscreen is a new root layer ("video fullscreen") that also makes the window fullscreen; Escape leaves both. YouTube and other iframe players still open their link.
- Mockcord: a 54 KB 3 s 320x180 H.264+AAC clip with Range support (`tools/mockcord/src/clip.rs`), posters, `post_video`, showcase "video" section.
- Tests: `timeline::video_tests` (in-process controls), bridge `video::a_gif_embed_plays_muted_loops_and_pauses_when_it_leaves_the_screen`, `video::a_video_attachment_streams_in_ranges_and_plays_pauses_seeks_and_ends`, `video::a_video_over_the_size_cap_and_an_expired_one_fail_without_playing`, `media_fetch::video_fetches_send_chromes_video_request`.
- Deviations: fullscreen is its own layer, not the image viewer; videos skip the disk cache (a new session downloads again); one volume and mute for every video, as in Discord; `gifv_request` kept only as the fallback for a GIF video outside the allow-list; mock posters are a gradient. Not checked on real Discord: attachment `url` played before `proxy_url`, the iframe-host list, and Chrome's later range requests (only `bytes=0-` was captured). Release exe size not measured.
- For later: at the Chrome 157 bump (74), capture an `<audio>`: kyoko's audio request still sends `accept-encoding: identity;q=1, *;q=0`, a guess; `--record` rewrites every kind, so record to a temporary name and copy only the wanted files. Fullscreen converts on the CPU at window size; hardware decode with a D3D11 device manager plus an NV12 zgx surface (103's note) would cut it.
