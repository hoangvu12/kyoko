# 07 — Gateway send window, heartbeat and reconnect policy

**What to build:** The gateway connection never floods Discord, notices when a connection has died, and reconnects correctly without looping on fatal errors.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** resolved

- [x] Outbound frames, heartbeats included, never exceed 120 per 60 seconds. A priority lane keeps heartbeats and urgent frames ahead of bulk work
- [x] The first heartbeat is jittered. A missing ACK closes the connection and resumes
- [x] The choice between resume and re-identify follows the session's state, and INVALID_SESSION is handled
- [x] Fatal close codes stop reconnecting and show a reason. Other drops back off, up to a cap
- [x] Session tests cover each case using mockcord's scenario controls

## Answer

Merged to main as `f573569..f9ff3e8`. Docs: `docs/gateway.md`, `docs/testing.md`.

- Send window (`crates/gateway/src/outbox.rs`): one sliding 120/60 s window per session, not reset on reconnect. Lanes: control (heartbeat, IDENTIFY, RESUME) may fill to 120, urgent (presence, voice state) to 110, bulk (guild subscriptions, member requests) to 100. Nothing is dropped; urgent queue 256, bulk 1024, `SendError::Full` beyond. Only the control lane is cleared on a new socket. `SendBudget` inside `GatewayTuning`, passed via `SessionConfig::gateway`.
- Heartbeat: first beat jittered; a missing ACK closes with 4000 and resumes. Op 9 false waits 1–5 s and identifies; op 9 true and op 7 resume.
- Fatal: 4004, 4010–4016, plus 1009. 4003/4007/4009 forget the session and identify. Backoff `1 s × 2^(n−1) × U(0.75,1.25)`, capped at 60 s, reset on READY/RESUMED.
- `state::ConnectionStatus { Connecting, Resuming, Connected, Reconnecting { attempt, retry_in }, Disconnected(DisconnectReason) }`, `DisconnectReason { AuthenticationFailed, Rejected { code, message } }`, at `SessionInfo::connection`. `LocalEvent::Connection(status)` replaces `ConnectionChanged`. 38 drives its banner from it; `AuthenticationFailed` routes to login (09/21).
- Transport already had zlib-stream with one inflate context; WebSocket message/frame and compressed-buffer caps raised to 64 MiB.
- Mockcord: `close_next_connections(code, count)`; `invalidate_session(false)` now ends the session server-side.
- Tests: 15 in `crates/session/tests/session/gateway.rs` plus a paused-clock outbox unit test; `cargo xtask test` also runs `-p kyoko-gateway`. Fatal-code tests use the one documented fixed sleep (250 ms). The suite takes about 5 s; one run under heavy parallel builds took 20 s, so watch for timing sensitivity.
- Known, not fixed: gateway events go out with `send().await` on a bounded channel, so a stalled consumer would also stall heartbeats.
- For 08: a refreshed token must reach `GatewayConfig.token` before the next RESUME. For 13: `ConnectionStatus` is never persisted. For 14: debounce op 37 before `subscribe_guild`; nothing coalesces in the queue.
