# Testing

Kyoko's behaviour tests drive a real `Session` against an in-process mockcord. They check two things: what the session publishes in its `state` snapshot, and what Discord would have seen on the wire.

## Running

```
cargo xtask test
```

This runs `cargo test -p kyoko-session -p kyoko-gateway -p mockcord`. CI runs the same tests on Windows as part of `cargo test --workspace --locked`. To run a single test: `cargo test -p kyoko-session --test session <name>`.

## Layout

All session tests live in a single test binary, `crates/session/tests/session/`:

- `main.rs` lists the modules.
- `harness.rs` holds `Running` and `Options`.
- Each other file holds one behaviour area: `hooks.rs`, `wire.rs`, and so on. Add a file per ticket area and list it in `main.rs`.

Keeping everything in one binary means one link step and a shared harness without dead-code warnings. Every harness helper must be used by at least one test. Add helpers when a test needs them, not before.

## Starting a test

```rust
let running = Running::start("unique-name").await;
let running = Running::start_with(Options::new("unique-name").hooks(hooks)).await;
```

`Running` starts mockcord on a random loopback port and opens a store in a scratch directory under the temp dir, named after the test and the process id. It sets the plaintext-token setting so the mock token never reaches Credential Manager, then starts a `Session` pointed at mockcord's REST and gateway URLs. Test names must be unique within the binary. Always finish with `running.stop().await`, which shuts the session down, stops mockcord and removes the scratch directory.

`Options` sets recording on and pins the fixture anchor (see "Determinism"). Its builders:

- `hooks(..)` installs session hooks.
- `heartbeat_ms(ms)` shortens mockcord's heartbeat interval.
- `gateway(GatewayTuning { backoff, send_budget })` shrinks the reconnect backoff or the send window, so tests can reach the cap or fill the window in under a second.
- `caches(CacheLimits { .. })` shrinks the cache limits (see `docs/caches.md`), so tests reach them with a few channels and pages.
- `remember_account(false)` starts the session the way the app does for a `KYOKO_TOKEN` sign-in: nothing is saved to the store.
- `rest(RestTuning { .. })` sets the REST pacing gaps and breaker timings (see `docs/rest.md`). Tests that don't measure pacing can zero the gaps.
- `buckets(limit, window)` sets mockcord's rate-limit headers: every route allows `limit` requests per `window` (5 per 2 s by default, `MockConfig::bucket_limit` and `bucket_window`). mockcord reports the headers but does not enforce them.
- `prepare(|mock| ..)` runs against mockcord's `Control` before the session starts, for example `transform_ready` for the very first READY.

## Waiting

Never sleep for a fixed time. Wait on an observable event:

- `running.until("what", |state| ...)` re-checks the predicate every time the session publishes changes, and panics with the description after `STEP_TIMEOUT` (15 s).
- `running.until_wire("what", |recording| Some(..))` re-checks every time mockcord records traffic, and returns the first `Some`.
- `running.restart()` shuts the session down and starts a new one on the same store and mockcord, for behaviour that must survive a restart.
- `running.connected()` waits for READY and a live connection. `running.identified_connection()` returns the id of the connection that sent the first IDENTIFY. `running.open_text_channel()` selects the first guild text channel and waits for its history to load.
- `running.eventually("what", || ...)` polls a condition every 10 ms until `STEP_TIMEOUT`. Use it only for effects that publish no state change, such as a token reaching `running.store()` or a hook recording a call.
- `state.session().connection` holds the `ConnectionStatus` (see `docs/gateway.md`). Wait on it to see reconnect attempts, their `retry_in`, and stop reasons.

Wait on the effect you are about to assert. For example, wait for a RESUME frame on a new connection, not for "connected", which was already true before the drop.

## The wire

`running.mock()` returns mockcord's `Control` handle. Recording is off unless `MockConfig::record` is set or `Control::set_recording(true)` is called. The harness turns it on.

`Control::recording()` returns a `Recording` snapshot with:

- `requests`: every REST request under `/api/v<N>`, with fields `method`, `version`, `path` (without the `/api/vN` prefix), `query`, the full `headers`, the raw `body`, the response `status`, `injected`, and an arrival `at` (`std::time::Instant`). Helpers: `header(name)` (case-insensitive), `json()`, `is(method, pattern)`.
- `connections`: every gateway WebSocket upgrade, with its `query` and handshake `headers`.
- `frames`: every frame the client sent, as `connection`, `op`, decoded `data` and `at`. `opcode()` maps to `protocol::Opcode`.
- `closes`: close frames the client sent, with the code.

`Recording::requests_to(method, pattern)` and `frames_with(opcode)` filter. Patterns compare path segments. `*` or `{name}` matches one segment, `**` matches any number, and `"*"` as the method matches any method. Use the `at` timestamps to assert spacing and pacing. Group frames by `connection` to tell an IDENTIFY on a fresh socket from a RESUME after a drop.

Recorded traffic includes the Authorization header and the token in IDENTIFY. It stays in memory: never write a recording to disk or to logs.

## Injection

All injection goes through `Control`, and works whether or not recording is on.

REST replies are matched by method and pattern, in the order they were queued, before mockcord's auth check:

- `respond_once`, `respond_times(n)`, `respond_always`, `clear_replies`.
- `Reply::unauthorized()`, `forbidden()`, `missing_permissions()`.
- `Reply::rate_limited(retry_after, global)` gives a 429 with Discord's body, a `retry-after` header, `x-ratelimit-scope`, and `x-ratelimit-global` when `global` is set.
- `Reply::captcha_required()` and `captcha(keys)` give a 400 with Discord's captcha fields (`captcha_key`, `captcha_sitekey`, `captcha_service`, `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id`).
- `Reply::json(status, body)` and `error(status, code, message)` build anything else. `with_header` adds headers.

Gateway controls:

- `close_gateway(code)` closes every identified connection with the code: 4000 for a resumable drop, 4004 and other fatal codes for fatal closes.
- `close_next_identify(code)` closes the next IDENTIFY with the code instead of sending READY.
- `close_next_connections(code, count)` closes the next `count` new sockets with the code before HELLO. Use it for consecutive failed reconnects.
- `fail_next_resume()` answers the next RESUME with op 9 `false`.
- `invalidate_session(resumable)` sends op 9. With `false`, mockcord also ends the session, so the client can IDENTIFY again on the same socket. `request_reconnect()` sends op 7. `request_heartbeat()` sends op 1.
- `set_heartbeat_acks(HeartbeatAcks::Prompt | Delayed(d) | Withheld)` controls op 11 replies.
- `post_message(channel_id, author_id, content)` posts a message as any fixture user, stores it like a REST send and broadcasts MESSAGE_CREATE. It returns the new id. Use it for incoming messages and for the user's own messages without a send.
- `dispatch(event, data)` sends a dispatch to every session, with sequence numbers, buffered for resume. Use it for late private channels and other events that arrive after READY.
- `transform_ready(|identify, ready| ..)` edits READY before it is sent. It sees the IDENTIFY payload, so READY can depend on the advertised capabilities. `reset_ready()` removes it.

mockcord serves REST under both `/api/v9` and `/api/v10`. `Server::rest_base_url()` is the v9 base, and `Server::rest_base_url_for(version)` builds either. READY reports the `v` the gateway URL asked for.

mockcord also stands in for the parts of the web client's identity that Discord hands out:

- `/app` and `/assets/*.js` serve a page whose sentry script carries `MockConfig::build_number`. The harness points the session's build-number discovery at `Server::app_url()`, so no test reaches discord.com.
- Every API response sets a `__dcfduid` cookie (`device_cookie_value(seed)`) until the client sends it back.
- READY carries `apex_experiments.installation` (`installation_id(seed)`).

## Tokens and capabilities

mockcord accepts one token at a time for REST, IDENTIFY and RESUME. It starts as `MockConfig::token`, which `Server::token()` returns. A READY whose transform sets `auth_token` issues that token: mockcord accepts only the new one from then on, so a client still using the old token gets a 401 or a 4004 close. mockcord drops `auth_token` from READY when IDENTIFY did not advertise AUTH_TOKEN_REFRESH.

mockcord shapes its replies by the capabilities in IDENTIFY, so tests prove each bit's effect:

- VERSIONED_READ_STATES and VERSIONED_USER_GUILD_SETTINGS: containers, otherwise plain arrays.
- NON_CHANNEL_READ_STATES: one guild-event read state per guild.
- DEDUPE_USER_OBJECTS: READY `users` plus recipient ids, otherwise full `recipients` and relationship `user` objects.
- PRIORITIZED_READY_PAYLOAD: presences in READY_SUPPLEMENTAL, otherwise in each READY guild.
- LAZY_USER_NOTES: without it, READY carries `notes`.
- DEBOUNCE_MESSAGE_REACTIONS: reaction adds go out as MESSAGE_REACTION_ADD_MANY, otherwise as MESSAGE_REACTION_ADD.

## Time

Two approaches apply, depending on the code under test.

- **Session tests use real time.** The session runs on its own runtime and talks to mockcord over real loopback sockets. The REST limiter and client measure with `std::time::Instant`. Tokio's paused clock would either not apply or would fire timeouts while sockets are still busy. So session tests:
  - stay event-driven, using `until` and `until_wire`;
  - make the waits they provoke short, for example `Reply::rate_limited(0.25, false)`, `HeartbeatAcks::Delayed(..)`, or a small `heartbeat_interval_ms` (the gateway clamps intervals to at least 100 ms);
  - assert durations as lower bounds from recorded `at` timestamps, never as upper bounds.

  Waits the session chooses itself, such as gateway reconnect backoff (about 1 s on the first failure by default) and the 1 to 5 s identify delay after op 9 `false`, fit inside `STEP_TIMEOUT`. Shorten the backoff with `Options::gateway` when a test provokes several failures.

  To prove something did not happen, such as no reconnect after a fatal close, wait a short, fixed time that is several times the tuned backoff cap. This is the one place a fixed sleep is acceptable.
- **Paused clock for pure tokio-time logic.** Code that only uses `tokio::time`, with no sockets, can be tested with `#[tokio::test(start_paused = true)]` and `tokio::time::advance`. An example is the gateway outbox test in `crates/gateway/src/outbox.rs`. Prefer this for unit-level timing logic when a ticket asks for such a test.

## Determinism

Fixtures come from `MockConfig::seed`. The same seed and anchor always give the same guilds, channels, users and messages. The harness pins `MockConfig::anchor` to 2026-01-01T00:00:00Z, so snowflakes and timestamps don't drift with the wall clock. Without an anchor, mockcord rounds the current time down to the hour. Messages created during a test get fresh snowflakes, newer than every fixture message. `live_traffic` and `chaos` add random traffic and must stay off in behaviour tests.

## The headless driver

`cargo xtask headless-run --mock` runs a longer scripted session against mockcord and checks counts and features. It can also run with `--chaos`. It complements the behaviour tests and should keep passing. Use the behaviour tests for assertions about specific traffic.
