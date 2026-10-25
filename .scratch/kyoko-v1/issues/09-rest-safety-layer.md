# 09 — REST safety layer

**What to build:** Write actions are paced and Discord's limits are respected. Failures stop rather than escalate, and carry a clear reason the UI can show.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** resolved

- [x] Rate-limit buckets are learned from response headers. A 429 honours retry_after and the global flag
- [x] Mutations are spaced out, and message sends have their own pacing
- [x] A 401 stops all requests and signals sign-out
- [x] Repeated 403s on a route trip a breaker for that route
- [x] A captcha response becomes a structured error and is never retried automatically
- [x] Every failed send carries a reason: rate limit, permission, captcha or network
- [x] Session tests assert each behaviour on the wire

## Answer

Merged to main as `942bfdc..b777fdf` (after the zgx move). Docs: `docs/rest.md`.

- `crates/rest/src/limiter.rs`: buckets learned only from headers, keyed with the route's major parameter; one in-flight probe per route until its bucket is known; a 429 pauses the route, a global 429 pauses everything; out-of-order responses keep the lower `remaining`. A 429 is retried at most once; a 429 on a non-idempotent POST is not retried.
- `crates/rest/src/safety/`: `stop.rs` (401 stop), `pacing.rs` (reserved send slots), `breaker.rs` (per-route 403 breaker keyed with the major id, half-open trial after cool-down), `mod.rs` (classification). Order in `RestClient::execute`: stop → bucket → pacing → global pause → breaker → send. `RestClient::stop_all/is_stopped/stopped`.
- `RestError` gained `MissingPermissions`, `Suspended { retry_in, missing_permissions }`, `Captcha { service }` (service name only). `protocol::rest::CaptchaChallenge` decodes captcha fields leniently.
- `rest::RestTuning` in `RestConfig::tuning` and `SessionConfig::rest`: `mutation_gap` 250 ms, `send_gap` 500 ms, `channel_send_gap` 1 s, `forbidden_strikes` 3, `forbidden_window` 60 s, `breaker_cool_down` 5 min, `max_retry_wait` 60 s.
- `state::SendFailure { RateLimited { retry_after }, Forbidden, MissingPermissions, Captcha { service }, Unauthorized, Network, Other(Option<u16>) }` on `PendingMessage` and `MessageView::failure`; `LocalEvent::PendingFailed` carries it.
- A 401 shuts the gateway, publishes `Disconnected(AuthenticationFailed)` and ignores later gateway events; a gateway 4004 also stops REST. A 401 does not wipe the stored token (ticket 21 decides).
- Mockcord `MockConfig::bucket_limit`/`bucket_window`; harness `Options::rest(..)`, `Options::buckets(..)`. 8 tests in `tests/session/safety.rs`, each verified to fail with its mechanism disabled. No `Network` test (mockcord can't drop a connection).
- Fixed the flaky send-window test (bounds by send order from `burst_began`, heartbeat interval 450 ms): 30/30 and 25/25 under parallel load.
- For 10: acks and single deletes are mutations and get `mutation_gap`; debounce acks before REST. 11: locks live in the session before REST. 17: backfill GETs aren't paced but get probe and buckets; never retry after `Suspended`/`RateLimited`. 21: route to login on `AuthenticationFailed`. 27: show `MessageView::failure`; for `Captcha`, say "complete this in the official client".
