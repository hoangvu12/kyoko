# 18 — Media pipeline

**What to build:** Images, avatars, emoji and stickers load efficiently at every size kyoko shows, including animated ones, without memory growing forever.

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** ready-for-agent

- [ ] PNG, JPEG, GIF, WebP and animated WebP decode in Rust and reach zgui as host images
- [ ] A byte-budgeted LRU bounds decoded images. Animations pause offscreen and honour the animation setting
- [ ] CDN and media-proxy URLs request device-pixel sizes, and an original-resolution option exists
- [ ] Expired signed attachment URLs are refreshed by re-fetching the message
- [ ] Fetching uses one pooled client with bounded concurrency
