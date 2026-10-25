# 18 — Media pipeline

**What to build:** Images, avatars, emoji and stickers load efficiently at every size kyoko shows, including animated ones, without memory growing forever.

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** ready-for-agent

- [ ] PNG, JPEG, GIF, WebP and animated WebP decode in Rust and reach zgx as host images
- [ ] A byte-budgeted LRU bounds decoded images. Animations pause offscreen and honour the animation setting
- [ ] CDN and media-proxy URLs request device-pixel sizes, and an original-resolution option exists
- [ ] Expired signed attachment URLs are refreshed by re-fetching the message
- [ ] Fetching uses one pooled client with bounded concurrency

## Comments

From the spike (04, `docs/spike.md`): send Chrome's image request headers on CDN fetches (the spike sent only a user agent); test sizing on a HiDPI display (only scale 1.0 was seen); pre-round avatars in Rust, because zgx's `rounded` does not clip images. The spike's avatar path (`media` → `ScriptHost::image`) took 72–125 ms on mock and 200–300 ms from the real CDN.
