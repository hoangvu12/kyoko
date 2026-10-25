# 17 — Fetch missing replies and users

**What to build:** Replies whose original message isn't loaded, and mentions of users kyoko hasn't seen, resolve automatically through polite background fetches. This is the built-in version of ValidReply and ValidUser.

**Blocked by:** 09 (REST safety layer)

**Status:** claimed

- [ ] Missing referenced messages are fetched, cached and paced
- [ ] Unknown mentioned users are fetched, cached and paced
- [ ] Fetch failures don't retry in a tight loop
- [ ] Session tests assert the backfill traffic and the resulting state
