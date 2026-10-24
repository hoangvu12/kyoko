# Gateway connection

`crates/gateway` owns one WebSocket at a time and keeps the session alive across drops. The session crate turns its events into `state::ConnectionStatus`.

## Transport

- JSON encoding with `compress=zlib-stream`. Each connection gets its own inflate context. A message is complete when the buffered bytes end in `00 00 ff ff`.
- Inbound WebSocket messages, buffered compressed bytes and inflated output are each capped at 64 MiB. A user READY can pass 16 MiB. Going over the cap closes with 1009 and stops the session with a reason.

## Send window

Every outbound frame counts against one sliding window: `SendBudget::frames` per `SendBudget::period`, 120 per 60 s by default. Heartbeats, IDENTIFY and RESUME count too. Frames go out in three lanes, highest priority first:

| Lane | Frames | Can fill the window up to |
|---|---|---|
| control | heartbeat, IDENTIFY, RESUME | all of it (120) |
| urgent | presence, voice state, streams | all but a twelfth (110) |
| bulk | guild subscriptions, member requests | all but a sixth (100) |

Bulk work can use at most 100 slots, so heartbeats and the handshake always have room, and a bulk burst never holds back a heartbeat. Blocked frames wait in their queue. They are not dropped, and frames queued during a reconnect are sent once the next READY or RESUMED arrives. Only the control lane is cleared on a new socket. If a lane's queue is full, the handle returns `SendError::Full` and the caller logs it.

## Heartbeat

The first beat goes out at `interval × random[0, 1)` after HELLO, then one every interval. If a beat comes due before the previous one was ACKed, the client closes with 4000 and resumes. A server op 1 is answered at once.

## Resume or identify

The client resumes only when it has a session id, a resume URL and a sequence number. Otherwise it identifies.

- op 7: close with 4000, reconnect to the resume URL, RESUME.
- op 9 `d:true`: the same as op 7.
- op 9 `d:false`: forget the session, wait 1 to 5 s at random, then IDENTIFY on the same socket.
- Close 4003, 4007 or 4009: forget the session, reconnect and IDENTIFY.

## Close codes and backoff

4004 and 4010 to 4016 are fatal. The client stops reconnecting and publishes `ConnectionStatus::Disconnected`. 4004 maps to `DisconnectReason::AuthenticationFailed`, so the UI can send the user back to login. Other fatal stops become `Rejected { code, message }`.

Every other drop reconnects after `Backoff::delay(failures)`. The delay is `base × 2^(failures-1)`, times a random factor between 0.75 and 1.25, and never more than `cap` (1 s and 60 s by default). An op 7 or a resumable op 9 counts as no failure, so the reconnect is immediate. The failure count resets on READY or RESUMED.

## Connection status

`state.session().connection` is one of:

- `Connecting` or `Resuming`: a socket is opening or the handshake is in flight.
- `Connected`: READY or RESUMED arrived.
- `Reconnecting { attempt, retry_in }`: the last socket dropped, and the next try comes after `retry_in`.
- `Disconnected(reason)`: stopped for good.

`state.session().connected` is true exactly when the status is `Connected`.

`SessionConfig::gateway` takes a `GatewayTuning` with the backoff and send budget. Production uses the defaults. Tests shrink them.
