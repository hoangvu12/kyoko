# Gateway connection

`crates/gateway` owns one WebSocket at a time and keeps the session alive across drops. The session crate turns its events into `state::ConnectionStatus`.

## Transport

- JSON encoding with `compress=zlib-stream`. Each connection gets its own inflate context. A message is complete when the buffered bytes end in `00 00 ff ff`.
- Inbound WebSocket messages, buffered compressed bytes and inflated output are each capped at 64 MiB. A user READY can pass 16 MiB. Going over the cap closes with 1009 and stops the session with a reason.

## Capabilities

IDENTIFY's `capabilities` is `gateway::ADVERTISED_CAPABILITIES`, built from the named bits in `protocol::capability`. A bit is advertised only when kyoko fully handles what it changes. The value is 509 until the real-traffic spike (ticket 04) compares it with the web client; change it there and nowhere else.

| Bit | Name | Advertised | Why |
|---|---|---|---|
| 0 | LAZY_USER_NOTES | yes | READY leaves out `notes`. Kyoko doesn't show notes. |
| 1 | NO_AFFINE_USER_IDS | no | Changes which presences and members arrive for implicit relationships. Not needed, and untested against real traffic. |
| 2 | VERSIONED_READ_STATES | yes | `read_state` is an `{entries, version, partial}` container. `EntryContainer` reads both shapes. |
| 3 | VERSIONED_USER_GUILD_SETTINGS | yes | The same container for `user_guild_settings`. |
| 4 | DEDUPE_USER_OBJECTS | yes | Users come once in READY `users`. DMs carry `recipient_ids`, relationships carry ids. State resolves both. |
| 5 | PRIORITIZED_READY_PAYLOAD | yes | Presences, merged members and voice states arrive in READY_SUPPLEMENTAL, which state applies. |
| 6 | MULTIPLE_GUILD_EXPERIMENT_POPULATIONS | yes | Only changes the experiments shape. Kyoko reads no experiments. |
| 7 | NON_CHANNEL_READ_STATES | yes | `read_state` gains entries for guild events, the notification centre and so on. State keeps only channel entries. |
| 8 | AUTH_TOKEN_REFRESH | yes | READY may carry `auth_token`. See "Refreshed token" below. |
| 9 | USER_SETTINGS_PROTO | no | READY drops the JSON `user_settings`. Kyoko decodes only guild folders from the protobuf. |
| 10 | CLIENT_STATE_V2 | no | Undocumented. |
| 11 | PASSIVE_GUILD_UPDATE | no | Replaces CHANNEL_UNREAD_UPDATE for unsubscribed guilds with PASSIVE_UPDATE_V1, which kyoko doesn't parse. |
| 12 | AUTO_CALL_CONNECT | no | Sends CALL_CREATE for existing calls without op 13. Kyoko has no calls. |
| 13 | DEBOUNCE_MESSAGE_REACTIONS | no | Batches reactions into MESSAGE_REACTION_ADD_MANY, which kyoko doesn't parse. |
| 14 | PASSIVE_GUILD_UPDATE_V2 | no | Same as bit 11, with PASSIVE_UPDATE_V2. |
| 15 | CHANNEL_OBFUSCATION | no | Undocumented. |
| 16 | AUTO_LOBBY_CONNECT | no | Kyoko has no lobbies. |

Batched reactions and passive guild updates are left off rather than implemented, because their payload shapes are only documented by the community. Unbatched reactions give the same snapshot. Without the passive bits, Discord sends CHANNEL_UNREAD_UPDATE for unsubscribed guilds instead. Kyoko doesn't parse that event yet either, so unread state for those guilds can go stale until the read-state work handles it.

## Refreshed token

When READY carries a non-empty string `auth_token`, it replaces the token everywhere:

- The gateway swaps `GatewayConfig.token` before it handles READY, so every later RESUME and IDENTIFY uses the new token.
- It then emits `GatewayEvent::TokenRefreshed` ahead of `GatewayEvent::Ready`. The session hands the token to the REST client before it applies READY, so every request after READY uses it.
- When `SessionConfig::remember_account` is true, the session saves it with `store.tokens` (keyring, or the opted-in plaintext file) and re-saves the account entry under `store.accounts`. When it is false, the refreshed token stays in memory, like the original, and nothing reaches the store.
- Saves and sign-out share one lock and a signed-in flag. Sign-out clears the flag before it removes the token and the account, so a save that runs after sign-out writes nothing.

`auth_token` is read on its own with `protocol::refreshed_token`. A missing, empty or non-string value is ignored and never fails READY. The token is never logged.

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
