# 121 — Paste and drop files anywhere (from oxidcord b3c036b)

**What to build:** Port oxidcord `b3c036b`: paste images/files from the clipboard and drop files anywhere on the chat column, thread panel and voice chat; the forum post dialog gets the same; the message editor no longer pastes file paths; uploads get a 15 minute timeout, files read off the UI thread with a 500 MiB cap, more extensions get a proper content type, animated GIF emoji stay animated, sticker uploads labelled with their real type. Adapt to kyoko; compare first. User approved (2026-10-29).

**Status:** resolved

- [x] Paste and drop work like Discord in the composer, thread panel, forum post dialog and voice chat; tests drive it in-process (no OS clipboard or drag in tests)
- [x] Upload timeout, size cap, content types, GIF emoji and sticker types as upstream, with session tests
- [x] Commit bodies end `Ported from oxidcord b3c036b...` (full sha)

## Answer

Merged `8493aaf` (lane B, worker `08f4b83d`; `6c25d2b` Give uploads 15 minutes and type more files, `8493aaf` Attach pasted and dropped files in every composer), verified by the orchestrator: all done-means steps green on the first run; tls-capture chrome-156 0/0; `parity mockup` composer-drop 11.2%, composer-uploads 18.5% (card and tray match in place and size; the rest is mock content).

- Root cause: Windows OLE drags give no position (winit ignores the POINTL, zgx reuses the last cursor), so every drop went to the main composer; uploads used the 30 s API timeout or a 300 s PUT timeout; the forum post dialog had no paste, drop or size check; server-settings picks flattened GIFs to PNG on the UI thread.
- REST: `RestConfig.timeout` is gone; use `RestTuning::request_timeout` (30 s) and `RestTuning::upload_timeout` (15 min, upload PUTs and every multipart API request). Content types in `rest::content_type` / `UploadFile::named` (adds apng, jfif, avif, heic, svg, m4v, mkv, avi, opus, m4a, aac, flac).
- Drops: `drops::accept_files` on the Rust root routes by pane under the cursor (thread panel -> thread composer, full call view -> the voice channel's chat, forum -> new post form, else main composer); the "Upload to ..." card names the target and follows the OS cursor on the composer's 250 ms tick (`platform::pointer::over_own_window`; Windows sends no drag-over). New drop target = a `DropTarget` variant plus a `DropZones` field. The 108 DM call view sits above the DM chat, so its drops go to the main composer.
- Paste: forum post body (`forum.paste`) attaches files and pictures; while editing a message a file paste is swallowed and a picture paste does nothing (no path text). Pasted images re-encode losslessly to PNG (upstream's keep-original-bytes skipped: Windows hands over pixels).
- Reads off the UI thread (`incoming::read_upload`, clipboard via `incoming::pasted(&SystemClipboard)`): folders, files over the channel's limit or 500 MiB refused before reading; more files than free slots refused as a whole; a file read for a channel the composer left is dropped. Server settings: GIF emoji (<= 256 KB) and GIF/APNG stickers (<= 512 KB) sent as they are (`bridge::prepared_image`); pickers filtered. `bridge::NO_ATTACH_PERMISSION` exported; captures use `Link::file_drag(FileDrag::Over(..))`.
- Tests: `drops::tests` (2), `ui::scripts::paste`, `composer::slow_uploads_outlast_the_request_timeout_and_keep_their_content_types`, `uploads::a_slow_gif_sticker_outlasts_the_request_timeout_as_a_gif_but_a_json_write_does_not`, bridge `guild_admin::picked_emoji_and_stickers_keep_their_animation_and_stills_are_fitted_as_png`.
- Deviations: kyoko keeps its window-wide "Upload to #channel" card (matches the Midnight mockup) rather than a full-column overlay; a forum drop opens the new-post form; no parity screen for a drop over the thread panel.