# 05 — Session test harness on mockcord

**What to build:** Contributors can write behaviour tests that drive a session against an in-process mockcord. A test checks both what the session publishes and what Discord would have seen on the wire, and can inject failures.

**Blocked by:** 02 (Strip GPUI UI, voice, WASM plugins and updater)

**Status:** resolved

- [x] Mockcord records the gateway frames and REST requests it receives (method, route, headers, body, timing), and tests can query them
- [x] Tests can inject 401, 403, 429 with retry-after, captcha errors, close codes, resume failures and late-arriving dispatches
- [x] Tests run on seeded fixtures with controllable time, and are deterministic
- [x] A first set of tests covers login through READY to the guild list appearing in the snapshot, and a sent message observed on the wire
- [x] One command runs the suite, and CI runs it on Windows

## Answer

Merged to main as `89daca5..563f1b9`. `cargo xtask test` runs `cargo test -p kyoko-session -p mockcord` (about 1.5 s for the session suite); CI covers it through `cargo test --workspace --locked`. Guide: `docs/testing.md`.

- Session tests live in one binary, `crates/session/tests/session/` (`harness`, `hooks`, `wire`). `Running::start("name")` / `Running::start_with(Options::new("name").hooks(..))` starts mockcord with recording on, a scratch store and the session. End with `running.stop().await`.
- `running.mock()` is mockcord's `Control`. `.recording()` holds `requests` (method, API version, path, query, headers, body, status, injected flag, time), `connections` (handshake headers and query), `frames` (opcode, JSON, connection id, time) and `closes`. Filters: `requests_to("POST", "/channels/{id}/messages")`, `frames_with(Opcode::Identify)`; `*`/`{x}` match one segment, `**` any number. Recording is in memory only and off unless `MockConfig::record`.
- Injection: `respond_once/times/always` with `Reply::unauthorized`, `forbidden`, `missing_permissions`, `rate_limited(retry_after, global)`, `captcha_required`/`captcha`; gateway `close_gateway(code)`, `close_next_identify(code)`, `fail_next_resume()`, `invalidate_session(resumable)`, `request_reconnect()`, `request_heartbeat()`, `set_heartbeat_acks(Prompt | Delayed(d) | Withheld)`; `dispatch(event, json)` (survives resume) and `transform_ready(|identify, ready| ..)`.
- Mockcord serves `/api/v9` and `/api/v10` (`rest_base_url_for(9)`) and never checks capabilities.
- Waiting: `until(what, |state| ..)` and `until_wire(what, |recording| ..)`, 15 s timeout, no fixed sleeps. Session tests use real time (real sockets); assert durations as lower bounds from recorded timestamps. Tokio's paused clock suits logic that only uses tokio time (e.g. the send-window limiter).
- Fixed: mockcord presence timestamps used the wall clock; fixtures now use the fixture date (determinism test added).
- Notes: 06: gateway still hardcodes `v=10` (`crates/gateway/src/transport.rs`) and mockcord's READY reports `"v": 10`. 07: heartbeat interval clamps to at least 100 ms; after op 9 false the client waits 1–5 s. 08: mockcord accepts only `config.token`; teach it the refreshed token. 13: add a restart-on-same-store helper with its first user. Add harness helpers only with a test that uses them (dead code fails clippy).
