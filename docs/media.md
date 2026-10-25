# Media

Images, avatars, emoji and stickers are fetched, decoded and cached in Rust (`crates/media`) and reach the window only as zgx host images (`ScriptHost::image`). zgx's own remote loader (`ui.image { src = "https://…" }`) is never used: it never evicts and spawns a thread per decode. The UI never touches the network; it asks for images by request and shows them by name.

## The gallery

`media::Gallery` is the one object the UI side talks to. It is `Send + Sync`, cheap to clone, and lives as long as the window.

- `Gallery::request(ImageRequest) -> ImageName` is cheap and safe to call on every render. It returns a stable name such as `media/3f2a…` and starts a load if the image is not resident. The window shows it as `ui.image { src = "host:" .. name }`. The same URL and variant always give the same name, and a signed attachment keeps its name when its signature is refreshed (the key ignores `ex`, `is` and `hm`).
- `ImageRequest` is a URL plus a `Variant`: a `SizeHint` in device pixels and a `Shape` (`Square`, or `Circle` for avatars, masked in Rust). A zgx node with `rounded` and `overflow = "hidden"` does clip images inside it, which is how the server rail rounds `Square` icons. `ImageRequest::from_message(Origin { channel_id, message_id })` marks an attachment so it can be refreshed.
- `Gallery::drain(now) -> Vec<Update>` runs on the UI thread. Each `Update::Show { name, pixels }` becomes `host.image(name, Some(ImageData))` and each `Update::Clear { name }` becomes `host.image(name, None)`. Pixels are an `Arc<[u8]>` shared with the gallery, so handing them to zgx copies nothing.
- `Gallery::set_waker(f)` is called whenever a drain has work: an image loaded or was evicted, or an animation frame is due. The desktop pushes into a `bridge::Inbox<()>` and a UI task drains the gallery on each wake.
- `Gallery::set_visible(names)` replaces the set of images on screen. Visible images animate and are never evicted. The desktop passes one union of everything on screen (the timeline's rows, the user panel avatar, the rail's icons, Luau's report); Luau can report a virtualized list's rows with `ui.call_sync("media_visible", names)`.
- `Gallery::set_hovered(name)`, `set_playback(Playback)` and `set_reduced_motion(bool)` drive animation. Luau reports hover with `ui.call_sync("media_hover", name)` and the system's reduced-motion preference with `ui.call_sync("media_reduced_motion", ui.prefers.reduced_motion:get())`.
- `Gallery::status(name)` and `stats()` are for tests and diagnostics.

Request builders for what kyoko shows live in `bridge`: `avatar_request`, `guild_icon_request`, `emoji_request`, `attachment_request`, `embed_request` (embed images and thumbnails: through the media proxy with `width`/`height`, WebP unless a GIF), `embed_icon_request` and `sticker_request` (PNG and APNG from the CDN, GIF through the proxy, none for Lottie). In mock runs `bridge::local_media` sends every URL that is not on the mock server to its `/media/<hash of the path>.png`, the same name oxidcord's mock resolver uses, and `MediaFetcherBuilder::only_loopback` drops the Discord hosts from the mock fetcher, so a mock run never contacts Discord. `avatar_request` falls back to the default avatar (`<cdn>/embed/avatars/<n>.png`) for a user without an avatar hash, as the web client does; mockcord serves those too. `bridge::media_fetcher` builds the fetcher from the fingerprint.

## Memory

Decoded frames are kept as RGBA in one LRU bounded by bytes: `DEFAULT_BUDGET_BYTES`, 128 MiB, set from `settings.media.memory_mb` and changeable with `Gallery::set_budget`. Every request and every visibility report makes an image the most recently used, so the order is "least recently shown". Over budget, the least recently shown images that are not visible go first, and the window is told to drop each one it holds. Visible images are never evicted; if the visible images alone exceed the budget, a newly loaded image that is not visible yet is dropped instead.

An animation keeps all its frames only while they fit in an eighth of the budget (`ANIMATION_SHARE`); a larger one keeps its first frame. Decoding limits (32 Mpx per image, 500 frames, 64 Mpx across all frames, 64 MiB input) stop hostile files earlier. At most 4096 entries are remembered, failed ones included.

The on-disk cache (`<cache>/media/<2 hex>/<sha256>`) is pruned by the store, see `docs/storage.md`. The gallery tolerates it vanishing at sign-out: a missing file is a miss and the next write recreates the folder.

## Fetching and decoding

- One pooled `reqwest` client, HTTPS only (loopback HTTP for mockcord), redirects re-checked against a host allowlist, bodies capped at 64 MiB.
- At most 6 requests in flight (`fetch::DEFAULT_CONCURRENCY`). Two requests for the same URL share one download.
- Decoding runs on the blocking pool, at most 2 at a time (`DEFAULT_DECODE_WORKERS`). PNG and APNG, JPEG, GIF, WebP and animated WebP decode with the `image` crate. Images larger than twice the hint are shrunk to it.
- A failed load is not retried for 60 s, unless it is a refreshable attachment (below).

Headers are Chrome's for an `<img>` on discord.com, from `Fingerprint::image_headers` (see `docs/fingerprint.md`): the Chrome user agent, `sec-ch-ua`, `sec-ch-ua-mobile`, `sec-ch-ua-platform`, `Accept: image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8`, `Accept-Encoding`, `Accept-Language`, `Referer: https://discord.com/`, `Sec-Fetch-Dest: image`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site` and `Priority: i`. There is no Authorization header and no cookie. Discord's cookies are scoped to discord.com, so a browser never sends them to `cdn.discordapp.com` or `media.discordapp.net`, and kyoko's jar stays with the API client.

## Sizes

All sizes are device pixels: points times the display scale, rounded up.

- Avatars, server icons, emoji, banners and role icons use the CDN's `size=`, a power of two from 16 to 4096 (`cdn::bucket`). Avatars and icons are `.webp`; an animated (`a_`) hash adds `animated=true` when the caller wants it animated, otherwise the CDN sends a still. Emoji add `quality=lossless`.
- Attachments go through the media proxy (`proxy_url`, or a `cdn.discordapp.com/attachments` URL rewritten to `media.discordapp.net`). The image is fitted inside the box the caller passes, never upscaled, and asked for as `format=webp&quality=lossless&width=W&height=H`, keeping the signature. GIFs keep their format and only add the size; an attachment flagged animated adds `animated=true`.
- The original-resolution setting (`settings.media.original_resolution`, `cdn::Quality::Original`) fetches the attachment's original `url` with no resize parameters. It replaces Vencord's FixImagesQuality. Memory stays bounded: the decoded image is still shrunk to the box when it is more than twice as large.

## Animation

One ticker task sleeps until the next frame of any animating image, then wakes the window, which drains the gallery. An image animates only when it is visible, `Playback` allows it (`Always`; `OnHover` while hovered; never with `Never`) and reduced motion is off. Reduced motion is on when kyoko's `accessibility.reduced_motion` or the system preference is set. Scrolled away, an animation pauses on its frame; when motion is not allowed it returns to its first frame. Finite GIF loop counts are honoured. A frame that is long overdue (the window was busy) restarts the timing rather than racing through the missed frames.

## Expired attachment links

Attachment URLs are signed: `ex` is the expiry in hex seconds, with `is` and `hm`. When a request's `ex` is in the past, the gallery reads the disk cache only and never contacts the CDN. When the CDN answers 403, 404 or 410 for a signed URL, the same applies. In both cases the gallery calls its refresher once with the message's `Origin`, and the desktop sends `Command::RefreshMedia { channel_id, message_id }` to the session. The session queues it on the backfill queue (`docs/rest.md`), which fetches `GET /channels/<c>/messages?around=<m>&limit=1` at its usual pace. `LocalEvent::MediaRefreshed` then replaces the message's attachments and embeds in the channel window and in fetched reply originals. The view re-renders, asks for the fresh URL under the same name, and the image loads. If the fresh URL fails too, the image is marked failed; nothing loops. Stale URLs are never retried, and full signed URLs are never logged (`redact_url` drops the query).
