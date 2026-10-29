# 121 — Paste and drop files anywhere (from oxidcord b3c036b)

**What to build:** Port oxidcord `b3c036b`: paste images/files from the clipboard and drop files anywhere on the chat column, thread panel and voice chat; the forum post dialog gets the same; the message editor no longer pastes file paths; uploads get a 15 minute timeout, files read off the UI thread with a 500 MiB cap, more extensions get a proper content type, animated GIF emoji stay animated, sticker uploads labelled with their real type. Adapt to kyoko; compare first. User approved (2026-10-29).

**Blocked by:** 120 (same lane, run after)

**Status:** ready-for-agent

- [ ] Paste and drop work like Discord in the composer, thread panel, forum post dialog and voice chat; tests drive it in-process (no OS clipboard or drag in tests)
- [ ] Upload timeout, size cap, content types, GIF emoji and sticker types as upstream, with session tests
- [ ] Commit bodies end `Ported from oxidcord b3c036b...` (full sha)