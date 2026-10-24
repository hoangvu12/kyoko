# 16 — Keep deleted and edited messages

**What to build:** Deleted messages stay visible in place with a deleted flag, and earlier versions of edited messages are kept, within limits, ready for MessageLogger.

**Blocked by:** 12 (Bounded state caches), 15 (Store hardening)

**Status:** ready-for-agent

- [ ] Deleted messages are kept in place with a deleted flag instead of being removed
- [ ] Previous versions of edited messages are kept
- [ ] Retention is capped per channel and by age, and wiped on sign-out
- [ ] State exposes the kept versions for rendering
- [ ] Session tests cover delete and edit sequences and the caps
