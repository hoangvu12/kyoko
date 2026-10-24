# 09 — REST safety layer

**What to build:** Write actions are paced and Discord's limits are respected. Failures stop rather than escalate, and carry a clear reason the UI can show.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** ready-for-agent

- [ ] Rate-limit buckets are learned from response headers. A 429 honours retry_after and the global flag
- [ ] Mutations are spaced out, and message sends have their own pacing
- [ ] A 401 stops all requests and signals sign-out
- [ ] Repeated 403s on a route trip a breaker for that route
- [ ] A captcha response becomes a structured error and is never retried automatically
- [ ] Every failed send carries a reason: rate limit, permission, captcha or network
- [ ] Session tests assert each behaviour on the wire
