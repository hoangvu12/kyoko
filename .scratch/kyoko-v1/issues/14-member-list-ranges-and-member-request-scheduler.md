# 14 — Member list ranges and member-request scheduler

**What to build:** The member list of any server can be browsed in full as the user scrolls, using Discord's lazy subscriptions politely.

**Blocked by:** 07 (Gateway send window, heartbeat and reconnect policy)

**Status:** ready-for-agent

- [ ] Guild subscriptions use op 37, debounced and deduplicated
- [ ] Member-list ranges are requested in 100-row buckets around the viewport, not only the first 100
- [ ] All member-list update ops are applied: SYNC, INSERT, UPDATE, DELETE and INVALIDATE
- [ ] Member requests (op 8) are scheduled and respect rate-limited responses
- [ ] Session tests assert the subscriptions and the resulting member-list state
