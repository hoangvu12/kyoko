# 18 — Media pipeline

**What to build:** Images, avatars, emoji and stickers load efficiently at every size kyoko shows, including animated ones, without memory growing forever.

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** resolved

- [x] PNG, JPEG, GIF, WebP and animated WebP decode in Rust and reach zgx as host images
- [x] A byte-budgeted LRU bounds decoded images. Animations pause offscreen and honour the animation setting
- [x] CDN and media-proxy URLs request device-pixel sizes, and an original-resolution option exists
- [x] Expired signed attachment URLs are refreshed by re-fetching the message
- [x] Fetching uses one pooled client with bounded concurrency

## Comments

From the spike (04, `docs/spike.md`): send Chrome's image request headers on CDN fetches (the spike sent only a user agent); test sizing on a HiDPI display (only scale 1.0 was seen); pre-round avatars in Rust, because zgx's `rounded` does not clip images. The spike's avatar path (`media` → `ScriptHost::image`) took 72–125 ms on mock and 200–300 ms from the real CDN.

## Answer

Merged to main as `10b37dc..07b8abb` (rebased over 16 by the orchestrator; union conflict in `LocalEvent`). Docs: `docs/media.md`.

- One object, `media::Gallery`: `gallery.request(ImageRequest) -> ImageName` (stable `media/<16 hex>`, cheap per render; Luau `ui.image { src = "host:" .. name }`). `ImageRequest` = URL + `Variant` (device-pixel size, `Square`/`Circle`); attachments add `.from_message(Origin { channel_id, message_id })`. Signed-URL params are ignored in the key. `set_waker`, `drain(now) -> Vec<Update::{Show{name,pixels}, Clear{name}}>` on the UI thread → `host.image(name, ..)`. `set_visible(names)` (replaces the set), `set_hovered`, `set_playback(Always|OnHover|Never)`, `set_reduced_motion`, `set_budget`, `next_due`, `status`, `stats`. Luau `ui.call_sync("media_visible"|"media_hover"|"media_reduced_motion", ..)`.
- Bridge builders: `avatar_request`, `guild_icon_request`, `emoji_request`, `attachment_request(att, channel, message, box_points, scale, Quality)`, `refresh_command`, `media_fetcher(&fingerprint, loopback)`.
- Defaults: 128 MiB RGBA (`settings.media.memory_mb`); an animation keeps all frames only within 1/8 of the budget, else its first frame; ≤4096 entries; failed loads retried after 60 s; 6 fetches, 2 decode workers, same URL fetched once. Visible images are never evicted.
- Sizing: device pixels → CDN `size=` power-of-two buckets; avatars/icons `.webp` (+`animated=true` only when wanted); emoji `quality=lossless`; attachments via the media proxy `format=webp&quality=lossless&width&height`, never upscaled; GIFs keep format. `settings.media.original_resolution` (`Quality::Original`) fetches the original, shrunk in memory beyond 2× the box. Avatars are circle-masked in Rust.
- Headers: `Fingerprint::image_headers()` (Chrome UA, `sec-ch-ua*`, image `Accept`, `Accept-Encoding`, `Accept-Language`, `Referer: https://discord.com/`, `Sec-Fetch-Dest: image`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site`, `Priority: i`). No Authorization, no cookies on CDN fetches.
- Expired URLs: `ex` in the past or 403/404/410 on a signed URL → disk cache only, one refresh via `Command::RefreshMedia { channel_id, message_id }` on 17's backfill queue → `LocalEvent::MediaRefreshed` → same name reloads; a second failure marks it failed. `Session::commander()` gives a cloneable `Commander`.
- Settings `settings.media { animation, original_resolution, memory_mb }`. Fixed the desktop disk-cache path so 15's pruning sees the files (`cache/media/<2 hex>/..`). `MediaService::load` returns `Decoded`, no tokio context needed.
- Mockcord serves PNG/JPEG/GIF/animated-WebP avatars and emoji, signed attachments that expire, records CDN requests (`Recording::media`), `Server::post_image`. 8 tests in `crates/bridge/tests/bridge/media.rs`. HiDPI tested only at the seam (1.25, 1.5).
- For 25/26: report visible rows via `set_visible` (merge in the user-panel avatar); use `attachment_request`; embeds need their own builder. For 32: `Quality::Original` with the window as the box. For 22/30: avatars `Circle`; server icons `Square` (the rail morph needs zgx clipping or two variants); avatars aren't re-requested on display-scale change. For 37: wire `media.*` and `accessibility.reduced_motion`. For 21: `set_budget(0)` or drop the gallery at sign-out.
