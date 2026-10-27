# Testing

Kyoko's behaviour tests drive a real `Session` against an in-process mockcord. They check two things: what the session publishes in its `state` snapshot, and what Discord would have seen on the wire.

## Running

```
cargo xtask test
```

This runs `cargo test -p kyoko-session -p kyoko-gateway -p mockcord -p kyoko-plugins` and the Luau load check. CI runs the same tests on Windows as part of `cargo test --workspace --locked`. To run a single test: `cargo test -p kyoko-session --test session <name>`.

## Layout

All session tests live in a single test binary, `crates/session/tests/session/`:

- `main.rs` lists the modules.
- `harness.rs` holds `Running` and `Options`.
- Each other file holds one behaviour area: `hooks.rs`, `wire.rs`, and so on. Add a file per ticket area and list it in `main.rs`.

Plugin tests are a second binary, `crates/plugins/tests/plugins/`, that includes this harness by path (`docs/plugins.md`). Their hosts load plugins with a 15 s `Limits::load_deadline` (`support::config`, and the plugin interface lab in `apps/desktop/src/ui/scripts/plugins.rs`), so a machine busy with other builds never fails a test on a slow load; the call deadlines stay as each test sets them, so overruns are still measured. Production keeps 1 s.

Keeping everything in one binary means one link step and a shared harness without dead-code warnings. Every harness helper must be used by at least one test. Add helpers when a test needs them, not before.

## Starting a test

```rust
let running = Running::start("unique-name").await;
let running = Running::start_with(Options::new("unique-name").hooks(hooks)).await;
```

`Running` starts mockcord on a random loopback port and opens a store in a scratch directory under the temp dir, named after the test and the process id. The store gets a `store::MemoryKeyring` instead of Credential Manager, so secrets take the production keyring path without touching the real one. It then starts a `Session` pointed at mockcord's REST and gateway URLs. Test names must be unique within the binary. Always finish with `running.stop().await`, which shuts the session down, stops mockcord and removes the scratch directory.

`Options` sets recording on and pins the fixture anchor (see "Determinism"). Its builders:

- `hooks(..)` installs session hooks.
- `heartbeat_ms(ms)` shortens mockcord's heartbeat interval.
- `gateway(GatewayTuning { backoff, send_budget })` shrinks the reconnect backoff or the send window, so tests can reach the cap or fill the window in under a second.
- `guilds(n)` sets how many guilds mockcord makes (3 by default; the first two are in the mock folder). mockcord keeps the folders a `PATCH /users/@me/settings-proto/1` writes and serves them in later READYs. It serves the frecency proto at `GET /users/@me/settings-proto/2` with favourite emoji seeded as the first server's `blob_wave` and `heart`, keeps a `PATCH /users/@me/settings-proto/2`, and `Control::set_favorite_emojis` and `update_favorite_emojis` (which also dispatches the type 2 update) change them. Mockcord's command index has the command mockups' apps: Mockbot (`/ping`, `/echo text language`, `/roll sides [count]`, `/broken`, and for M12 `/think` (defers with a `LOADING` reply, follows up after `InteractionTiming::followup`, 2 s; `--slow-followups` makes it ten minutes), `/whisper` (an ephemeral reply, not stored) and `/ignore` (never answers; `INTERACTION_FAILURE` after `InteractionTiming::window`, 3 s); replies carry the nonce and `interaction_metadata`; `--hold-ping` accepts `/ping` without any gateway event; `Control::set_interaction_timing`), Remindy (`/remind when message`), Polly (`/poll question`) and Snapper (`/snap`, `/resize file width`), with ranges and choices on the options (`/echo`'s six languages) and Mockbot's `/translate text language` answering autocomplete with the same languages (`--slow-autocomplete` answers after ten minutes, `--failing-autocomplete` refuses it with a 500, `Control::set_autocomplete` switches at run time); `/resize` claims its uploaded attachment and answers with the file's name; it keeps the frecency proto's `application_command_frecency`, and `--frequent-commands` (`MockConfig::frequent_commands`) seeds it with `/remind`, `/roll` and `/shrug`. Each app in the index carries the mockup's description. For M15 `--more-apps` (`MockConfig::more_apps`) adds the apps launcher mockup's Warden (`/warn reason`), Lyra (`/play song`), Ticketeer (`/ticket`) and Countr (`/count`), and Arcade, an activity-only app whose only command is a primary entry point (type 4), and seeds the frecency proto with `/roll`, `/remind`, `/poll`, `/snap`, `/play` and `/warn` instead. For M13 Mockbot's `/bug` opens a "Report a bug" modal (`INTERACTION_MODAL_CREATE`, then `INTERACTION_SUCCESS`) and answers its submit with "Thanks, filed: <title>" (`--failing-modals`, `Control::set_failing_modals`, fails every submit). Mockbot answers presses on its own messages: Primary, Secondary, Success and Celebrate post "You pressed <label>", Danger fails after `InteractionTiming::window`, Refresh succeeds after `InteractionTiming::followup`, the fruit selects succeed without a message, anything else fails. The component messages (`mockcord::components`) are in `#showcase`'s `showcase: components` section, and `Control::post_components` posts any raw components as Mockbot (unknown types included). For M14 its GIF search returns the GIFs whose names match first, then the rest, and its pack stickers `hello` and `bye` carry the tag `wave`.
- `members(n)` sets mockcord's members per guild, for member lists longer than a few buckets.
- `theme_links(ThemeLinkTuning { max_bytes, timeout, trusted_roots })` shrinks the theme link limits and trusts extra PEM roots. `theme_links.rs` serves theme files from a local HTTPS server with a root and server certificate made by `rcgen` for each test, so no key is committed.
- `caches(CacheLimits { .. })` shrinks the cache limits (see `docs/caches.md`), so tests reach them with a few channels and pages.
- `remember_account(false)` starts the session the way the app does for a `KYOKO_TOKEN` sign-in: nothing is saved to the store.
- `plaintext_secrets(true)` turns on the plaintext storage setting before the store opens (see `docs/storage.md`).
- `keyring(Arc<MemoryKeyring>)` passes a keyring the test prepared, for example with a seeded cache key or `set_failing(true)` / `set_failing_writes(true)` to simulate Credential Manager errors. `running.keyring()` returns it at any time.
- `retention(Retention { .. })` shrinks the pruning limits and interval, so pruning runs within a test.
- `seed(|paths| ..)` writes files into the scratch directory before the store opens, for example media cache entries through `media::DiskCache`.
- `rest(RestTuning { .. })` sets the REST pacing gaps and breaker timings (see `docs/rest.md`). Tests that don't measure pacing can zero the gaps.
- `buckets(limit, window)` sets mockcord's rate-limit headers: every route allows `limit` requests per `window` (5 per 2 s by default, `MockConfig::bucket_limit` and `bucket_window`). mockcord reports the headers but does not enforce them.
- `prepare(|mock| ..)` runs against mockcord's `Control` before the session starts, for example `transform_ready` for the very first READY.
- `clock(Clock)` gives the client identity a `protocol::fingerprint::Clock`, kept across restarts. Build one from a shared offset (`Clock::new(move || start + *elapsed.lock())`) and move the offset to age the heartbeat session id without waiting.

## Waiting

Never sleep for a fixed time. Wait on an observable event:

- `running.until("what", |state| ...)` re-checks the predicate every time the session publishes changes, and panics with the description after `STEP_TIMEOUT` (15 s).
- `running.until_wire("what", |recording| Some(..))` re-checks every time mockcord records traffic, and returns the first `Some`.
- `running.restart()` shuts the session down and starts a new one on the same store and mockcord, for behaviour that must survive a restart. `running.restart_after(|mock| ..)` runs the closure while no session is connected, for changes the next launch must catch up on. `running.restart_with_retention(..)` restarts with new pruning limits, for start-up pruning of what the first session cached.
- `running.scratch()` is the scratch root, to check that no file holds a secret.
- `running.connected()` waits for READY and a live connection. `running.identified_connection()` returns the id of the connection that sent the first IDENTIFY. `running.open_text_channel()` selects the first guild text channel and waits for its history to load.
- `running.eventually("what", || ...)` polls a condition every 10 ms until `STEP_TIMEOUT`. Use it only for effects that publish no state change, such as a token reaching `running.store()` or a hook recording a call.
- `state.session().connection` holds the `ConnectionStatus` (see `docs/gateway.md`). Wait on it to see reconnect attempts, their `retry_in`, and stop reasons.

Wait on the effect you are about to assert. For example, wait for a RESUME frame on a new connection, not for "connected", which was already true before the drop.

## The wire

`running.mock()` returns mockcord's `Control` handle. Recording is off unless `MockConfig::record` is set or `Control::set_recording(true)` is called. The harness turns it on.

`Control::recording()` returns a `Recording` snapshot with:

- `requests`: every REST request under `/api/v<N>`, with fields `method`, `version`, `path` (without the `/api/vN` prefix), `query`, the full `headers`, the raw `body`, the response `status`, `injected`, and an arrival `at` (`std::time::Instant`). Helpers: `header(name)` (case-insensitive), `json()`, `is(method, pattern)`.
- `connections`: every gateway WebSocket upgrade, with its `query` and handshake `headers`.
- `frames`: every frame the client sent, as `connection`, `op`, decoded `data`, the `raw` JSON text of `d`, and `at`. `opcode()` maps to `protocol::Opcode`. `data` sorts object keys, so assert key order with `keys(path)`, which lists the keys of the object at `path` in `raw` in wire order (`keys(&[])` for `d` itself, `keys(&["properties"])` for IDENTIFY's properties).
- `closes`: close frames the client sent, with the code.
- `media`: every request under `/cdn/`, the mock CDN, with `path`, `query`, `headers`, `status` and `at`. Helpers: `header(name)`, `param(name)`, `url()`. `Recording::media_to(prefix)` filters by path prefix.

`Recording::requests_to(method, pattern)` and `frames_with(opcode)` filter. Patterns compare path segments. `*` or `{name}` matches one segment, `**` matches any number, and `"*"` as the method matches any method. Use the `at` timestamps to assert spacing (see "Time" for pacing). Group frames by `connection` to tell an IDENTIFY on a fresh socket from a RESUME after a drop.

The mock CDN serves avatars (`/cdn/avatars/<user>/<hash>.<png|jpg|webp|gif>`; `a_` hashes are animated as GIF, or as WebP with `animated=true`, four frames of 100 ms), server icons (`/cdn/icons/<guild>/<hash>.<ext>`, generated like avatars), emoji (`/cdn/emojis/<id>.<ext>`) and signed attachments (`/cdn/attachments/<channel>/<id>/<file>`). Attachments answer 404 when `ex` has passed or `hm` does not match, honour `width`, `height` and `format=webp`, and are re-signed with a fresh expiry whenever a messages list is served. `Server::post_image(channel, filename, Signature::{Fresh, Expired, Revoked})` posts a message with one such attachment. Media tests live in `crates/bridge/tests/bridge/media.rs`.

Recorded traffic includes the Authorization header and the token in IDENTIFY. It stays in memory: never write a recording to disk or to logs.

## Injection

All injection goes through `Control`, and works whether or not recording is on.

REST replies are matched by method and pattern, in the order they were queued, before mockcord's auth check:

- `respond_once`, `respond_times(n)`, `respond_always`, `clear_replies`.
- `Reply::unauthorized()`, `forbidden()`, `missing_permissions()`.
- `Reply::rate_limited(retry_after, global)` gives a 429 with Discord's body, a `retry-after` header, `x-ratelimit-scope`, and `x-ratelimit-global` when `global` is set.
- `Reply::captcha_required()` and `captcha(keys)` give a 400 with Discord's captcha fields (`captcha_key`, `captcha_sitekey`, `captcha_service`, `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id`).
- `Reply::json(status, body)` and `error(status, code, message)` build anything else. `with_header` adds headers.
- `hold_requests(method, pattern)` keeps matching requests (`/upload/**` included) waiting before mockcord reads their bodies, until `release_requests()`; an upload test holds the second file's `PUT` to see the first counted as sent.
- Two upload names answer on their own, for captures that cannot reach `Control`: `mockcord::STALLED_UPLOAD` (`stalled-upload.mp4`) is not answered for ten minutes and `mockcord::BROKEN_UPLOAD` (`broken-upload.mp4`) gets a 500.

Gateway controls:

- `close_gateway(code)` closes every identified connection with the code: 4000 for a resumable drop, 4004 and other fatal codes for fatal closes.
- `close_next_identify(code)` closes the next IDENTIFY with the code instead of sending READY.
- `close_next_connections(code, count)` closes the next `count` new sockets with the code before HELLO. Use it for consecutive failed reconnects.
- `fail_next_resume()` answers the next RESUME with op 9 `false`.
- `invalidate_session(resumable)` sends op 9. With `false`, mockcord also ends the session, so the client can IDENTIFY again on the same socket. `request_reconnect()` sends op 7. `request_heartbeat()` sends op 1.
- `set_heartbeat_acks(HeartbeatAcks::Prompt | Delayed(d) | Withheld)` controls op 11 replies.
- `post_message(channel_id, author_id, content)` posts a message as any fixture user, stores it like a REST send and broadcasts MESSAGE_CREATE. It returns the new id. Use it for incoming messages and for the user's own messages without a send.
- `edit_message(channel_id, id, content)` edits any stored message as if its author did, sets `edited_timestamp` and broadcasts MESSAGE_UPDATE. `delete_message(channel_id, id)` and `bulk_delete(channel_id, ids)` remove messages as another client or a moderator would and broadcast MESSAGE_DELETE or MESSAGE_DELETE_BULK. Each returns whether mockcord knew the target. A message the client posts containing `mockcord::MODERATED_PHRASE` ("spoiler alert") is deleted by mockcord 400 ms later, as a moderator bot would, so an app driving only its own REST calls can see someone else delete a message (the `message-logger-deleted` capture uses it).
- `rate_limit_member_requests(retry_after, count)` answers the next `count` op 8 requests with a RATE_LIMITED dispatch (`opcode` 8, `retry_after`, `meta.guild_id` and `meta.nonce`) instead of GUILD_MEMBERS_CHUNK.
- `dispatch(event, data)` sends a dispatch to every session, with sequence numbers, buffered for resume. Use it for late private channels and other events that arrive after READY.
- `transform_ready(|identify, ready| ..)` edits READY before it is sent. It sees the IDENTIFY payload, so READY can depend on the advertised capabilities. `reset_ready()` removes it.

mockcord serves REST under both `/api/v9` and `/api/v10`. `Server::rest_base_url()` is the v9 base, and `Server::rest_base_url_for(version)` builds either. READY reports the `v` the gateway URL asked for.

mockcord answers op 37 with a GUILD_MEMBER_LIST_UPDATE that SYNCs every range listed for the guild, list id `everyone`, with an online and an offline group. It answers op 8 with one GUILD_MEMBERS_CHUNK per guild, echoing the nonce. Kicking or banning a listed member sends a list DELETE and an UPDATE of its group header, as well as GUILD_MEMBER_REMOVE. To test other list ops, send them with `dispatch("GUILD_MEMBER_LIST_UPDATE", ..)`.

mockcord also stands in for the parts of the web client's identity that Discord hands out:

- `/app` and `/assets/*.js` serve a page whose sentry script carries `MockConfig::build_number`. The harness points the session's build-number discovery at `Server::app_url()`, so no test reaches discord.com.
- Every API response sets a `__dcfduid` cookie (`device_cookie_value(seed)`) until the client sends it back.
- READY carries `apex_experiments.installation` (`installation_id(seed)`) and `api_code_version` (`API_CODE_VERSION`).

## Tokens and capabilities

mockcord also has Discord's remote-auth gateway for QR login at `/remote-auth` (`Server::remote_auth_url()`, `?v=2`) and the ticket exchange at `POST /users/@me/remote-auth/login`, outside the auth check. It plays the real handshake with RSA-OAEP and closes 4001 on a wrong nonce proof; `MockConfig::remote_auth_timeout_ms` (2 minutes) closes an unanswered code with 4003. A test plays the phone with `Control::scan_qr_code()` (`pending_ticket` with the current user), `confirm_qr_login()` (`pending_login`, then a ticket the exchange turns into the current token), `cancel_qr_login()`, `expire_qr_code()` and `garble_next_qr_nonce()`; `qr_codes_shown()` counts codes waiting on a phone and `qr_connections()` gives each socket's request headers. A `phone=scan`, `phone=login` or `phone=expire` query on the URL plays those steps by itself, for capture mode and the headless run. A captcha on the exchange is a queued `respond_once("POST", "/users/@me/remote-auth/login", Reply::captcha_required())`. See `docs/sign-in.md`, "QR login".

For web login mockcord serves a login page at `/login` (`Server::login_page_url()`). It sets `__dcfduid`, `__sdcfduid` and `_cfuvid` (`login_cookies(seed)`), posts `POST /api/v9/auth/login` as `mock@kyoko.invalid` / `mock-password` (below) and keeps the token in `localStorage` as Discord's page does, then goes to `/app`; `?auto=1` logs in by itself. The seam tests fetch the page and post the form over HTTP, then hand `WebSignIn` the page's message and cookies; the real page in the WebView runs in the `login-web` capture. `web_page::agent` in `apps/desktop` opens a real WebView2 page through `PageHost` on a hidden window against its own loopback server (not mockcord, whose recording covers only `/api`) and checks that the user agent and client hints agree and are WebView2's; it needs the WebView2 Runtime, as the app does.

The email form's routes play Discord's account side, outside the auth check, with the shapes of docs.discord.food's Authentication page: `POST /auth/login`, `POST /auth/mfa/sms/send` and `POST /auth/mfa/{totp,sms,backup}` (`tools/mockcord/src/password.rs`). The password is `MOCK_PASSWORD` (`mock-password`) for every account, and the login picks the behaviour: `PASSWORD_LOGIN` (`mock@kyoko.invalid`) gets the current token at once; `MFA_LOGIN` (`mfa@kyoko.invalid`) gets `{mfa: true, ticket, login_instance_id, totp, sms, backup}` with all three methods; `SECURITY_KEY_LOGIN` (`key@kyoko.invalid`) gets a ticket with only a `webauthn` challenge; `CAPTCHA_LOGIN` (`captcha@kyoko.invalid`) gets `Reply::captcha_required()`. Anything else, or a wrong password, is the 50035 form error `INVALID_LOGIN` "Login or password is invalid." on `login` and `password`. Codes: `TOTP_CODE` `123456` (the TOTP route also takes the backup code, as Discord's does), `SMS_CODE` `246810` only after `sms/send` for that ticket (which answers `{phone: "+*******0085"}`), `BACKUP_CODE` `kyoko123`; a wrong code is 60008 "Invalid two-factor code", an unknown ticket a form error, and `LIMITED_CODE` `000000` a 429, for the `login-2fa` capture. A good code ends the ticket. `Control::mfa_tickets_issued()` and `mfa_texts_sent()` count tickets and texts. Captchas and rate limits elsewhere are queued replies as usual, for example `respond_once("POST", "/auth/login", Reply::captcha_required())`. See `docs/sign-in.md`, "Email and password".

mockcord also accepts a second account's token, `MockConfig::alt_token` (`Server::alt_token()`, `mock-alt-token`). It signs in as `ALT_USER_ID` (`mockalt`) with no servers, DMs or friends: `GET /users/@me` answers with that user, READY carries only it, and broadcast events never reach its gateway sessions. Other routes answer with the main account's data. Tests use it to switch between two accounts.

For the main account, mockcord accepts one token at a time for REST, IDENTIFY and RESUME. It starts as `MockConfig::token`, which `Server::token()` returns. A READY whose transform sets `auth_token` issues that token: mockcord accepts only the new one from then on, so a client still using the old token gets a 401 or a 4004 close. mockcord drops `auth_token` from READY when IDENTIFY did not advertise AUTH_TOKEN_REFRESH.

mockcord shapes its replies by the capabilities in IDENTIFY, so tests prove each bit's effect:

- VERSIONED_READ_STATES and VERSIONED_USER_GUILD_SETTINGS: containers, otherwise plain arrays. Each container has a version that every ack, own message, mention and guild-settings patch bumps, and MESSAGE_ACK carries the new version. When IDENTIFY's `client_state` names a version mockcord has reached, the container is `partial: true` with only the entries changed since; otherwise it is full.
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
  - assert durations as lower bounds from recorded `at` timestamps, never as upper bounds;
  - never assert the gap between two arrivals when the wait starts at the first request's send rather than its response: a request delayed in transit on a busy machine shrinks the next gap. For REST pacing use `running.assert_released_apart(&requests, gap, what)`, which checks the pacer's release slots (`Session::releases()`, see `docs/rest.md`) are at least `gap` apart and that each request arrived no earlier than its slot. For other start-to-start queues (backfill), bound the k-th arrival by the trigger time plus k gaps. Waits that start at a response (a 429's `retry_after`, the server-write queue) may compare arrivals directly.

  Waits the session chooses itself, such as gateway reconnect backoff (about 1 s on the first failure by default) and the 1 to 5 s identify delay after op 9 `false`, fit inside `STEP_TIMEOUT`. Shorten the backoff with `Options::gateway` when a test provokes several failures.

  To prove something did not happen, such as no reconnect after a fatal close, wait a short, fixed time that is several times the tuned backoff cap. This is the one place a fixed sleep is acceptable.
- **Paused clock for pure tokio-time logic.** Code that only uses `tokio::time`, with no sockets, can be tested with `#[tokio::test(start_paused = true)]` and `tokio::time::advance`. An example is the gateway outbox test in `crates/gateway/src/outbox.rs`. Prefer this for unit-level timing logic when a ticket asks for such a test.

## Determinism

Fixtures come from `MockConfig::seed`. The same seed and anchor always give the same guilds, channels, users and messages. The harness pins `MockConfig::anchor` to 2026-01-01T00:00:00Z, so snowflakes and timestamps don't drift with the wall clock. Without an anchor, mockcord rounds the current time down to the hour. Messages created during a test get fresh snowflakes, newer than every fixture message. `live_traffic` and `chaos` add random traffic and must stay off in behaviour tests.

## The headless driver

`cargo xtask headless-run --mock` runs a longer scripted session against mockcord and checks counts and features. It can also run with `--chaos`. Its mockcord records traffic, so its wire checks (no typing request while SilentTyping is on, nothing naming a hidden channel, the bulk ack from ReadAllNotificationsButton) see real requests. It complements the behaviour tests and should keep passing. Use the behaviour tests for assertions about specific traffic.
