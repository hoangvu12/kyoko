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
| `delete_gap` | 1.2 s | between two message deletes in any channels |
| `bulk_ack_gap` | 2 s | between two `POST /read-states/ack-bulk` requests |

A message send is `POST /channels/<id>/messages`. The defaults sit between how fast a person acts and what Discord tolerates:

- 250 ms matches Discord's own reaction limit (about one per 250 ms). It stays below what a person notices when clicking reactions or deleting messages.
- Concord spaces mutations 500 ms apart and sends 100 ms apart. kyoko makes sends the strictest lane instead. Nobody sends two messages within half a second, and Discord's per-channel message bucket is about 5 per 5 s.
- Deletes get their own, stricter lane. Deleting many messages is the pattern self-bot cleaners show, so kyoko never deletes faster than about one message per second, even when the user selects many at once. 1.2 s keeps a ten-message cleanup around ten seconds.
- Mark server read and mark everything read send one bulk ack per 100 channels. 2 s between them spreads a large "mark all read" over a few seconds instead of a burst.
- Pacing only ever delays. Nothing is dropped or merged, so a burst of user actions still completes, just at a human pace.

## Read acks

Every read receipt goes through one debouncer per channel in the session (`crates/session/src/task/reads.rs`). The local read state updates immediately, so badges clear at once. The REST ack waits.

- A trigger schedules `POST /channels/<c>/messages/<m>/ack` with `{"token":null,"last_viewed":<days since the Discord epoch>}` after `ack_delay` (1.5 s) plus a random `ack_jitter` (up to 1 s). Another trigger for the same channel restarts the wait and raises the message id, so a burst gives one ack for the newest message. `ack_max_delay` (10 s) caps how long a busy channel can defer, so an open, busy channel sends at most about one ack every 10 s.
- Triggers: opening a channel, a new message in the open channel, `Command::AckChannel` and `Command::MarkChannelRead`.
- A new message only counts when the user is viewing the channel: it is the selected channel, the UI has not reported `Command::SetViewing(false)` (default: viewing), and the last `SetViewport` for it, if any, is at the bottom. Reporting viewing again, or scrolling back to the bottom, acks what arrived meanwhile.
- `Command::MarkUnread` sends a manual ack at once and drops any pending ack for the channel. New messages in that channel are not acked again until the user opens it again.
- `Command::MarkGuildRead` and `Command::MarkAllRead` send `POST /read-states/ack-bulk` (the web client's own endpoint for marking a server read) with up to 100 `{"channel_id","message_id","read_state_type":0}` entries each, paced by `bulk_ack_gap`. They cover channels with a read state that are behind or have mentions, skipping categories and channels without View Channel. Mark all read includes DMs.
- Pending acks are not sent at shutdown. Discord keeps the older read position and the next READY brings it back.

The 1.5 s delay follows how people read: a message is acked once it has had time to be seen, and a fast conversation coalesces. Concord debounces for 1 s.

## Deleting messages

`Command::DeleteMessage` and `Command::DeleteMessages` go through one queue (`crates/session/src/task/deletes.rs`) that sends single `DELETE /channels/<c>/messages/<m>` requests one after another, paced by `delete_gap`. kyoko never calls `POST /channels/<c>/messages/bulk-delete`: user accounts cannot use it.

- Only the user's own messages (known from the cache) are queued, plus any message in a server channel where the user has Manage Messages. Others are marked `Refused` and the command reports a rejection.
- Progress is in `State::deletions()`: each `DeleteBatch` lists its messages with a `DeleteStatus` of `Queued`, `Deleted`, `Refused`, `Failed(SendFailure)` or `Cancelled`. The last 16 batches are kept.
- A 404 counts as deleted. Any other failure (403, 429, captcha, a suspended route, 401, network) stops the batch: that message is `Failed` and the rest are `Cancelled`, never sent.

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
