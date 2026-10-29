# Gateway connection

`crates/gateway` owns one WebSocket at a time and keeps the session alive across drops. The session crate turns its events into `state::ConnectionStatus`.

## Transport

- The socket is a `wreq` websocket opened by `net::ws::connect` (`crates/net`) on a client the gateway builds once from `net::client_builder(Destination::Fetch, trust)` and keeps across reconnects. Each upgrade uses `net::websocket_emulation()`, so the wss ClientHello is Chrome's: ALPN `http/1.1` only and no ALPS (`docs/fingerprint.md`, "TLS and HTTP/2 handshake"). `GatewayConfig::trust` is `Trust::Bundled` in the app; tests pass a localhost root.
- The upgrade carries `User-Agent`, `Origin: https://discord.com` and `Accept-Language` from the identify properties, as before; `net::ws::connect` adds Chrome's `Pragma: no-cache`, `Cache-Control: no-cache` and `Accept-Encoding: gzip, deflate, br, zstd`, and wreq adds `Host` and the websocket headers. They go out with Chrome's names, case and order (`net::UPGRADE_ORDER`), except that `Connection` keeps wreq's lowercase value `upgrade` and there is no `Sec-WebSocket-Extensions`, because kyoko can't do permessage-deflate (`docs/fingerprint.md`, "Header order"). Connecting times out after 30 s; an upgrade answered with anything but 101 is a lost connection that names the status.
- `wss` is required, `ws` only to a loopback host (mockcord).
- JSON encoding with `compress=zlib-stream`. Each connection gets its own inflate context. A message is complete when the buffered bytes end in `00 00 ff ff`.
- Kyoko inflates zlib-stream itself (`transport::ZlibStream`, flate2); wreq only hands over the binary messages.
- Inbound WebSocket frames and messages (wreq's `max_frame_size` and `max_message_size`), buffered compressed bytes and inflated output are each capped at 64 MiB. A user READY can pass 16 MiB. Going over the cap closes with 1009 and stops the session with a reason; wreq's capacity error is recognised by `net::ws::is_oversized`.

## Capabilities

IDENTIFY's `capabilities` is `gateway::ADVERTISED_CAPABILITIES`, built from the named bits in `protocol::capability`. A bit is advertised only when kyoko fully handles what it changes. The value is 509 until the real-traffic spike (ticket 04) compares it with the web client; change it there and nowhere else.

| Bit | Name | Advertised | Why |
|---|---|---|---|
| 0 | LAZY_USER_NOTES | yes | READY leaves out `notes`. Kyoko doesn't show notes. |
| 1 | NO_AFFINE_USER_IDS | no | Changes which presences and members arrive for implicit relationships. Not needed, and untested against real traffic. |
| 2 | VERSIONED_READ_STATES | yes | `read_state` is an `{entries, version, partial}` container. `EntryContainer` reads both shapes. |
| 3 | VERSIONED_USER_GUILD_SETTINGS | yes | The same container for `user_guild_settings`. |
| 4 | DEDUPE_USER_OBJECTS | yes | Users come once in READY `users`. DMs carry `recipient_ids`, relationships carry ids, `merged_members` carry `user_id` instead of `user` (`Member::id`). State resolves all three; mockcord sends the same shape. |
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

Reaction events are read leniently: `type` 1 (or, without a `type`, `burst: true`) is a super reaction, counted in `count_details.burst` and `me_burst` apart from normal ones; `burst_colors` on an add replaces the reaction's colours, and a bad element in it is dropped. Batched reactions and passive guild updates are left off rather than implemented, because their payload shapes are only documented by the community. Unbatched reactions give the same snapshot. Without the passive bits, Discord sends CHANNEL_UNREAD_UPDATE for unsubscribed guilds instead, which kyoko applies (see `docs/client-state.md`).

IDENTIFY's `client_state` comes from the persisted read-state and guild-settings versions plus READY's `api_code_version`; `docs/client-state.md` covers what is sent and how partial READY containers merge.

## IDENTIFY

`frames::identify` builds the payload in the web client's key order: `token`, `properties`, `compress`, `presence`, `capabilities`, `client_state`. There is no `large_threshold`. `presence` is `{"activities":[],"status":"unknown","since":0,"afk":false}` (`GatewayConfig::INITIAL_STATUS`). Kyoko reports no game, listening or watching activities: IDENTIFY never carries one, and a presence update (op 3) carries only the custom status (type 4). While "Share my activity" (`show_current_game`) is off, `publish_presence` drops every activity but the custom status, so an activity kyoko reports later is held back without another change (`status::with_activity_sharing_off_presence_carries_nothing_but_the_custom_status`). `properties` comes from `GatewayConfig::properties`, a `PropertiesSource` that the gateway reads on every connect and every IDENTIFY. `docs/fingerprint.md` covers the property set.

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

The connection banner (`bridge::ConnectionWatch`) shows "Reconnecting" only after a socket really closed (`Reconnecting`, then the `Connecting` or `Resuming` of the next attempt). A `Connecting` while the session is live, which is an op 9 Invalid Session answered by a fresh IDENTIFY on the same socket, keeps the banner hidden. Nothing a channel open does touches the connection status.

`SessionConfig::gateway` takes a `GatewayTuning` with the backoff and send budget. Production uses the defaults. Tests shrink them.

## Opening a channel

What goes out when a channel opens (`Command::SelectChannel`), as the web client does, and nothing more:

- One history page, `GET /channels/{id}/messages?limit=50`, only when the channel has nothing in memory. A cached page from disk shows first and the fetched page merges around it. Reopening a channel already in memory sends nothing.
- For a guild channel, the op 37 change for that channel (see below), batched with other changes. The first channel in a guild also turns on typing, threads and activities; Discord answers with THREAD_LIST_SYNC and the member list SYNC.
- The read ack for the channel being left, if it was still pending, and the ack for the opened channel once its newest message is on screen ("Read acks" in `docs/rest.md`).
- Backfill of missing reply originals and unknown mentions, paced ("Backfill" in `docs/rest.md`). No profile is fetched when a channel opens; profiles load when one is opened ("Profiles" in `docs/rest.md`).

The message pane shows the skeleton row (`MessageRow::Loading`) until the first page arrives. If the first page fails, the session records `LocalEvent::HistoryFailed` and the pane shows `MessageRow::Failed` with the reason and a Retry (`Command::RetryHistory`), which clears it and loads again. Opening the channel again retries too. The failure stays in the pane: the connection banner, the member list and the channel list are untouched.

Nothing outside the message pane moves. THREAD_LIST_SYNC brings every active thread of the subscribed channels; a thread first learnt that way stays out of the channel list unless the user has joined it (`GuildState::unlisted_threads`; Discord lists joined threads only), and the threads popout still shows it. The sidebar, rail and member list reuse the previous row value for every unchanged row (`apps/desktop/src/stable.rs`), because zgx's keyed `ui.each` rebuilds a row whose item is a new allocation even when it is equal. Unread and mention state lives in `sidebar.badges`, not in `sidebar.rows`, so reading a channel redraws its dot and badge without rebuilding its row.

## Lazy loading

Guild subscriptions and member requests are planned in the session (`crates/session/src/task/subscriptions.rs` and `member_requests.rs`) and leave through the bulk lane. Nothing is sent before READY. A new READY starts over, a RESUMED keeps everything.

### Guild subscriptions (op 37)

One planner builds every op 37 frame. Opening a guild channel (`SelectChannel`), `SubscribeGuild` and `SetMemberViewport { guild_id, channel_id, first, last }` only change what the session wants. The change is sent 150 ms after the last one, and at most 500 ms after the first, so a burst of changes becomes one frame. Several guilds share a frame.

The planner remembers what it last sent per guild and sends only the difference:

- A guild's first entry carries `typing`, `threads` and `activities` set to true, `member_updates` false, empty `members` and `thread_member_lists`, and `channels`.
- Later entries carry only `channels`. A channel the guild no longer lists goes out with `[]`, which drops its ranges. A guild whose wanted state hasn't changed is left out of the frame, and an empty frame is never sent.
- The open guild and the three most recently used others keep a member-list channel. When a guild falls out of that set, its channel goes out with `[]`. Its typing and activity flags stay on, because Discord treats those subscriptions as permanent.
- After a new READY, only the open guild is subscribed again.

Member-list ranges are 100-row buckets. A channel's ranges are always `[0,99]`, the buckets the viewport touches, and one bucket of lookahead. The lookahead goes toward whichever bucket edge is nearer the viewport, and downwards on a tie or at the top. The result is capped at three ranges, keeping `[0,99]` and the viewport first. A viewport of rows 450 to 470 gives `[[0,99],[400,499],[500,599]]`. Without a viewport, a channel gets `[[0,99]]`.

### Member lists

GUILD_MEMBER_LIST_UPDATE is applied to one list per guild, `GuildState::member_list`, identified by its list id. An update for another id replaces the list only if it carries a SYNC. Otherwise it is a late update for a list the session has stopped watching, and it is ignored. Ops:

- SYNC writes the range's items. If it returns fewer items than the range asked for, the list ends there.
- INSERT and DELETE shift every later row.
- UPDATE replaces one row.
- INVALIDATE clears its range to placeholders.
- Unknown ops are ignored. A bad item becomes an empty placeholder, so the positions of the other rows still line up.

GUILD_MEMBER_REMOVE no longer edits the list, because Discord also sends a list DELETE for it and the row would go twice. Member data goes into the guild's member cache. The list holds only ids and group headers.

`State::member_list(guild_id)` returns the `MemberList`, with `groups` (ids and counts), `row_count()` and `rows(first, last)`. `State::member_list_rows(guild_id, first, last)` returns one `Option<MemberListRow>` per row, where `None` is a row that has not loaded yet. `member_rows(guild_id)` still returns only the loaded rows.

### Member requests (op 8)

`Command::RequestMembers { guild_id, user_ids }`, `Command::SearchMembers { guild_id, query }` and `Core::request_members` inside the session all feed one scheduler:

- **Batching:** ids for a guild wait 50 ms, then go out in requests of at most 100 ids, as `{"guild_id":[id],"user_ids":[..],"presences":false,"nonce":".."}`. A search is `{"guild_id":[id],"query":"..","limit":100,"presences":true,"nonce":".."}`. A newer search replaces one that hasn't gone out yet.
- **Pacing:** each guild has at most one request waiting for its answer. A request is answered when the last GUILD_MEMBERS_CHUNK with its nonce arrives, or it is given up after 10 s.
- **Deduplication:** ids that are queued, in flight, or were answered in the last 60 s (found or `not_found`) are not requested again.
- **Rate limits:** a RATE_LIMITED dispatch for op 8 puts that request back at the front of its guild's queue. The guild is held for `retry_after` seconds, or 5 s if Discord gave none, and never more than 120 s.
- **Reconnects:** after a new READY, unanswered requests are queued again and holds are cleared.
