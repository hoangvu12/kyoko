# 120 — GIF picker performance (from oxidcord 88dced0, db5bfa6)

**What to build:** Port oxidcord `88dced0` (small GIF thumbnails: Tenor previews / tinygif or proxy-resized webp, stills decoded from the first frame only, animate on hover, hover requests jump the media queue) and `db5bfa6` (lazy-load GIF picker categories and favourites, trending in staged batches of 25, favourites stored in the user's Discord GIF favourites through the expressions REST endpoints, media downloads capped at six in flight with a bounded queue), adapted to kyoko (Luau picker, 93's media queue). Compare first; port only what kyoko lacks. User approved (2026-10-29).

**Blocked by:** 119 (same lane, run after)

**Status:** ready-for-agent

- [ ] Picker opens fast with small stills; animation on hover; categories and favourites fetched when opened
- [ ] GIF favourites read/written through Discord's own endpoints the way the web client does (check against the web client; record in `docs/rest.md`)
- [ ] Session tests against mockcord; the picker capture checked by eye; tls-capture unchanged
- [ ] Commit bodies end `Ported from oxidcord <full sha>`