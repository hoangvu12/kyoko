# 120 — GIF picker performance (from oxidcord 88dced0, db5bfa6)

**What to build:** Port oxidcord `88dced0` (small GIF thumbnails: Tenor previews / tinygif or proxy-resized webp, stills decoded from the first frame only, animate on hover, hover requests jump the media queue) and `db5bfa6` (lazy-load GIF picker categories and favourites, trending in staged batches of 25, favourites stored in the user's Discord GIF favourites through the expressions REST endpoints, media downloads capped at six in flight with a bounded queue), adapted to kyoko (Luau picker, 93's media queue). Compare first; port only what kyoko lacks. User approved (2026-10-29).

**Blocked by:** 119 (same lane, run after)

**Status:** resolved

- [x] Picker opens fast with small stills; animation on hover; categories and favourites fetched when opened
- [x] GIF favourites read/written through Discord's own endpoints the way the web client does (check against the web client; record in `docs/rest.md`)
- [x] Session tests against mockcord; the picker capture checked by eye; tls-capture unchanged
- [x] Commit bodies end `Ported from oxidcord <full sha>`

## Answer

Merged `77614d5` (lane C, worker `19b91c91`), verified by the orchestrator: all done-means steps green; tls-capture chrome-156 0/0; `parity mockup` gif-picker 26.8%, gif-picker-results 30.2%, built-in-gif 19.0% (layout, stills, hover ring and star match; the rest is mock artwork and sidebar data).

- Stills: Tenor preview (AAAAe .png), else Discord's `preview`, else a proxy WebP at tile size for media-proxy/cdn attachment GIFs, else the GIF; decoded first frame only (`Variant::first_frame()`, own cache key). Hover: Tenor tinygif (AAAAM .gif, from oxidcord, untested live), else proxy GIF at tile size, else `gif_src`; hover jumps the queue (`Rank::Urgent` via `Gallery::set_hovered`, inside 93's per-host queue; `Ticket::hurry()`). Lists build 25 tiles at a time as you scroll; only stills in view are marked visible. `/gifs/trending` only when the GIF tab first shows. Tenor mp4 favourites get a still and tinygif.
- Already in kyoko: favourites via FrecencyUserSettings `favorite_gifs` (GET/PATCH `/users/@me/settings-proto/2`, as the web client; recorded in docs/rest.md), lazy favourites, 6/100 in flight per host (93). Skipped: upstream's limit=25/50/75/100 re-requests (the web client makes one `/gifs/trending-gifs` request), `/users/@me/gifs/favorites` (not used by the web client), frame decimation and its cache budget (87 streams animations).
- Tests: `media_queue::a_hovered_image_jumps_ahead_of_the_visible_images_queued_before_it`, `media::gif_picker_tiles_show_a_small_first_frame_still_and_play_a_small_gif_on_hover`, `pickers::tests::a_long_gif_list_shows_a_batch_of_first_frame_stills_reveals_more_on_scroll_and_moves_on_hover`.
- Check on a real run: Klipy has no known small rendition (hover plays `gif_src`); the AAAAM tinygif code.
