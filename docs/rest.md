# REST safety layer

Every REST request goes through one path, `RestClient::execute` in `crates/rest/src/client.rs`. Before a request is sent, the client checks the stop flag, then the rate-limit buckets, then the pacing slots, and finally the route's 403 breaker. It sends the request, learns from the response headers, and reports the outcome back to the breaker. The parts live in their own modules:

- `limiter.rs`: rate-limit buckets, probes, route waits and the global pause.
- `safety/pacing.rs`: spacing for mutations and message sends.
- `safety/breaker.rs`: the per-route 403 circuit breaker.
- `safety/stop.rs`: the sign-out stop after a 401.
- `tuning.rs`: `RestTuning`, every interval in one struct. `RestConfig::tuning` and `SessionConfig::rest` carry it.

## Rate limits

- Buckets come only from response headers: `X-RateLimit-Bucket`, `-Limit`, `-Remaining` and `-Reset-After`. Nothing is hard-coded per route. A route key is the method plus the path, with the major parameter (`channels/<id>`, `guilds/<id>`, `webhooks/<id>`) kept and other ids, emoji and invite codes templated. A bucket is the learned hash plus the major parameter.
- Until a route's bucket is known, only one request on that route is in flight at a time (the probe). Routes whose responses never carry bucket headers stay serialised.
- Responses can arrive out of order. If a response belongs to the current window, the client keeps the lower `remaining` and the later reset. A reset more than half a window later counts as a new window. Every learned reset gets a 10 ms margin for header rounding.
- A 429 waits for `retry_after` from the body, else the `Retry-After` header, else 1 s, plus 50 ms. While it waits, every request on that route waits too. If `global` is set (body or `X-RateLimit-Global`), every route waits. A request that has passed pacing checks the global pause again before it is sent, so a pause doesn't release a queued burst all at once.
- A 429 is retried at most once, only for non-POST or idempotent requests (sends that carry a nonce), and only when the wait is at most `max_retry_wait`. After that it is surfaced as `RestError::RateLimited { retry_after }`.

## Pacing

Pacing works on slots reserved in request order. A request sleeps until its slot, then sends. GET and HEAD are never paced.

| Field | Default | Applies to |
|---|---|---|
| `mutation_gap` | 250 ms | between any two non-GET requests, sends included |
| `send_gap` | 500 ms | between two message sends in any channels |
| `channel_send_gap` | 1 s | between two message sends in one channel |

A message send is `POST /channels/<id>/messages`. The defaults sit between how fast a person acts and what Discord tolerates:

- 250 ms matches Discord's own reaction limit (about one per 250 ms). It stays below what a person notices when clicking reactions or deleting messages.
- Concord spaces mutations 500 ms apart and sends 100 ms apart. kyoko makes sends the strictest lane instead. Nobody sends two messages within half a second, and Discord's per-channel message bucket is about 5 per 5 s.
- Pacing only ever delays. Nothing is dropped or merged, so a burst of user actions still completes, just at a human pace.

## Stop on 401

A 401 from any request stops the client. That request, everything queued in pacing or bucket waits, and every later request fail immediately with `RestError::Unauthorized`, without reaching the network. A 401 is never retried. A gateway close with 4004 stops REST the same way. The session sees the stop (`RestClient::stopped()`) and shuts the gateway down. It publishes `ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed)` at `SessionInfo::connection` and ignores later gateway events. The stop is permanent for that client. Signing in again starts a new session.

## 403 breaker

Each route key has its own breaker. 403s within `forbidden_window` (60 s) are counted, and Missing Permissions (50013) counts as well. After `forbidden_strikes` (3) of them the route opens for `breaker_cool_down` (5 minutes). While it is open, requests on the route fail with `RestError::Suspended { retry_in, missing_permissions }`, without being sent.

After the cool-down, the breaker lets one request through:

- If it succeeds, the breaker resets.
- If it gets another 403, the route opens again.
- If it ends any other way, the next request gets the trial.

Any successful response on the route resets its count. Other routes are never affected.

## Captcha

A response body with `captcha_key` becomes `RestError::Captcha { service }`. It carries only the service name (for example `hcaptcha`). The site key, rqdata, rqtoken and session id are never kept or logged. A captcha is never retried, solved or sent anywhere. The UI tells the user to complete the action in the official client.

## Failed sends

A failed message send stays pending with `PendingMessage::failure` (and `MessageView::failure`) set to a `state::SendFailure`:

- `RateLimited { retry_after }`
- `Forbidden`
- `MissingPermissions` (a 50013, or a breaker opened by them)
- `Captcha { service }`
- `Unauthorized`
- `Network`
- `Other(status)`

The session never re-sends a failed message on its own.
