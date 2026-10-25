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
| `settings_delay` | 1 s | quiet time after the last server-order change before it is saved |
| `settings_gap` | 10 s | between two server-order saves |
| `collapse_gap` | 2 s | between two saves of collapsed categories |

A message send is `POST /channels/<id>/messages`, and creating a thread or a forum post (`POST /channels/<id>/threads` and `POST /channels/<id>/messages/<message>/threads`) counts as one. The defaults sit between how fast a person acts and what Discord tolerates:

- 250 ms matches Discord's own reaction limit (about one per 250 ms). It stays below what a person notices when clicking reactions or deleting messages.
- Concord spaces mutations 500 ms apart and sends 100 ms apart. kyoko makes sends the strictest lane instead. Nobody sends two messages within half a second, and Discord's per-channel message bucket is about 5 per 5 s.
- Deletes get their own, stricter lane. Deleting many messages is the pattern self-bot cleaners show, so kyoko never deletes faster than about one message per second, even when the user selects many at once. 1.2 s keeps a ten-message cleanup around ten seconds.
- Mark server read and mark everything read send one bulk ack per 100 channels. 2 s between them spreads a large "mark all read" over a few seconds instead of a burst.
- Pacing only ever delays. Nothing is dropped or merged, so a burst of user actions still completes, just at a human pace.

Reactions, pins and unpins are mutations. Before any request the session checks permissions: adding a reaction needs Read Message History, and Add Reactions when nobody has reacted with that emoji yet; pinning needs Manage Messages in a server. A refusal never reaches REST, and a REST failure is reported as a `CommandRejected` with the reason; neither is retried.

## Read acks

Every read receipt goes through one debouncer per channel in the session (`crates/session/src/task/reads.rs`). The local read state updates immediately, so badges clear at once. The REST ack waits.

- A trigger schedules `POST /channels/<c>/messages/<m>/ack` with `{"token":null,"last_viewed":<days since the Discord epoch>}` after `ack_delay` (1.5 s) plus a random `ack_jitter` (up to 1 s). Another trigger for the same channel restarts the wait and raises the message id, so a burst gives one ack for the newest message. `ack_max_delay` (10 s) caps how long a busy channel can defer, so an open, busy channel sends at most about one ack every 10 s.
- Triggers: opening a channel, a new message in the open channel, `Command::AckChannel` and `Command::MarkChannelRead`.
- A new message only counts when the user is viewing the channel: it is the selected channel, the UI has not reported `Command::SetViewing(false)` (default: viewing), and the last `SetViewport` for it, if any, is at the bottom. Reporting viewing again, or scrolling back to the bottom, acks what arrived meanwhile.
- `Command::MarkUnread` sends a manual ack at once and drops any pending ack for the channel. New messages in that channel are not acked again until the user opens it again.
- `Command::MarkGuildRead` and `Command::MarkAllRead` send `POST /read-states/ack-bulk` (the web client's own endpoint for marking a server read) with up to 100 `{"channel_id","message_id","read_state_type":0}` entries each, paced by `bulk_ack_gap`. They cover channels with a read state that are behind or have mentions, skipping categories and channels without View Channel. Mark all read includes DMs.
- Pending acks are not sent at shutdown. Discord keeps the older read position and the next READY brings it back.

The 1.5 s delay follows how people read: a message is acked once it has had time to be seen, and a fast conversation coalesces. Concord debounces for 1 s.

## Server order and folders

The web client keeps the rail's order and folders in the preloaded settings protobuf and saves them with `PATCH /users/@me/settings-proto/1` and `{"settings":"<base64>"}`. kyoko does the same; the legacy `PATCH /users/@me/settings` is not used for it.

- `Command::SetGuildLayout(Vec<RailEntry>)` updates the local layout at once and hands it to one writer (`crates/session/src/task/layout.rs`). The write waits `settings_delay` (1 s) after the last change, so a burst of drops is one request with the final layout, and two writes are at least `settings_gap` (10 s) apart, following Discord's guidance for settings the user changes often. A drop never causes more than one request. The request still goes through the mutation lane.
- The body is the minimal protobuf `protocol::encode_guild_folders` writes: field 14 (`guild_folders`) of `PreloadedUserSettings` with every rail entry as a `GuildFolder`. A real folder carries `id`, `name` and `color` as wrapped values; a server outside folders is a folder of one with no id, as the web client stores it. The deprecated `guild_positions` is left empty. A sent top-level field replaces the stored one, so the whole field is always sent.
- Discord answers with the merged settings and dispatches USER_SETTINGS_PROTO_UPDATE, which the state applies as it does READY's folders.
- If the request fails and no newer change is waiting, the rail goes back to the last saved layout and the session reports a rejection. A newer change waiting is sent at its time instead.
- A change not yet sent when kyoko quits is lost; the next READY brings back the saved order.

The settings-proto gateway capability (bit 9) is still not advertised: Discord sends `user_settings_proto` in READY and the update event without it, and the write path does not depend on it.

## Collapsed categories

A category's collapsed state is the `collapsed` flag of its channel override in the user guild settings, which is where the web client keeps it and where oxidcord reads it from. kyoko saves it there with `PATCH /users/@me/guilds/{guild_id}/settings` and `{"channel_overrides":{"<category_id>":{"collapsed":true}}}`.

- `Command::ToggleCategory { guild_id, category_id }` collapses or expands the category locally at once and hands the new value to one writer (`crates/session/src/task/collapse.rs`). The write waits `settings_delay` (1 s) after the last toggle and carries every category of one guild changed since the last write, with its final value; writes are at least `collapse_gap` (2 s) apart, one guild per request, through the mutation lane. Toggling back and forth quickly is one request.
- Discord dispatches USER_GUILD_SETTINGS_UPDATE with the merged overrides; the state applies it and the versioned container saved for the next start carries it (`docs/client-state.md`), so the collapse survives a restart. An update arriving while a newer toggle waits keeps the newer value on screen.
- If the request fails, the categories it carried go back to their earlier state, unless a newer toggle for them is waiting, and the session reports a rejection.
- A toggle not yet sent when kyoko quits is lost; the next READY brings back the saved state.

## Deleting messages

`Command::DeleteMessage` and `Command::DeleteMessages` go through one queue (`crates/session/src/task/deletes.rs`) that sends single `DELETE /channels/<c>/messages/<m>` requests one after another, paced by `delete_gap`. kyoko never calls `POST /channels/<c>/messages/bulk-delete`: user accounts cannot use it.

- Only the user's own messages (known from the cache) are queued, plus any message in a server channel where the user has Manage Messages. Others are marked `Refused` and the command reports a rejection.
- Progress is in `State::deletions()`: each `DeleteBatch` lists its messages with a `DeleteStatus` of `Queued`, `Deleted`, `Refused`, `Failed(SendFailure)` or `Cancelled`. The last 16 batches are kept.
- A 404 counts as deleted. Any other failure (403, 429, captcha, a suspended route, 401, network) stops the batch: that message is `Failed` and the rest are `Cancelled`, never sent.

## Backfill

Replies whose original is not loaded, and mentions of users kyoko has not seen, resolve through one background queue in the session (`crates/session/src/task/backfill.rs`). It is kyoko's built-in version of Vencord's ValidReply and ValidUser.

What is fetched:

- A reply (`type` 19, a `message_reference` in the same channel, not a forward) whose `referenced_message` Discord did not embed and whose original is neither in the channel window nor already fetched. kyoko asks for `GET /channels/<c>/messages?around=<m>&limit=1`, the messages list the web client reads, rather than the single-message route. A result whose id is not the original counts as missing.
- A user mentioned as `<@id>` in the content but absent from the message's `mentions` and from the user cache. Inside a guild the lookup is an op 8 request through the member request scheduler (`docs/gateway.md`); it batches, dedupes and backs off on its own. If op 8 reports the id in `not_found`, or the channel is a DM, kyoko asks `GET /users/<id>` through the queue.
- The author of a guild message that arrived without a `member` (history pages) and has no member entry: an op 8 request, so names and role colours resolve. Ids op 8 reported as not found are not asked again for the cool-down.

- A message whose signed attachment URL expired or was refused by the CDN, on `Command::RefreshMedia` from the media gallery (`docs/media.md`). The same `around` request fetches it again, and its attachments and embeds are replaced with the freshly signed ones. The job is dropped if the message has left both the channel window and the fetched originals.

Only the open channel is scanned: when it is selected, when a page loads and when a message arrives or is edited. Originals are only fetched where the user has View Channel and Read Message History. Selecting another channel drops the queued work of every other channel. A request already sent finishes.

The queue sends one request at a time:

| Field | Default | Meaning |
|---|---|---|
| `backfill_gap` | 500 ms | between the starts of two backfill requests |
| `backfill_per_minute` | 30 | backfill requests in any 60 s |
| `backfill_cool_down` | 5 min | how long a failed or missing target is not asked again |

At most 200 jobs wait; more are dropped and come back on a later scan. A target that is queued, in flight or in its cool-down is never queued twice, so many replies to one message give one request.

Failures never loop:

- 404, 403 and Missing Permissions mark the original unavailable (`ReplyTarget::Unavailable`); it is never fetched again while cached. A user that is not found is remembered for the cool-down.
- A 429 that the client's own retry could not absorb pauses the whole queue for `retry_after` (at least the gap) and puts the job back once. A second 429 drops it for the cool-down.
- A suspended route pauses the queue for the breaker's `retry_in`, a captcha for 10 minutes, a network or server error for 5 s; the job waits out the cool-down.
- A 401 stops the queue for good.

Backfill GETs go through the same client as everything else, so they also respect the route probe, the learned buckets and the 403 breaker.

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

The session never re-sends a failed message on its own. It keeps the last 16 failed sends (text, reply, ping choice and files) so the user can ask for them again: `Command::RetryMessage { channel_id, nonce }` drops the failed pending message and sends a new one (the lock check runs again), `Command::DiscardMessage { channel_id, nonce }` drops it. The composer shows the reason with Retry and Delete (`bridge::failure_text`); a captcha says to finish in the official client.

## Sending, replies and uploads

`Command::SendMessage { channel_id, content, reply_to, ping_reply, files }` sends what the user wrote. A reply carries `message_reference` with `fail_if_not_exists: false`. With the ping turned off it also carries `allowed_mentions: {"parse": ["users", "roles", "everyone"], "replied_user": false}`, as the web client sends; with it on, `allowed_mentions` is left out. A send resets the typing throttle for the channel, so the next keystroke posts typing again.

Files go the web client's way: `POST /channels/{id}/attachments` with `{"files": [{id, filename, file_size, is_clip}]}` returns an upload slot per file, each file is `PUT` to its `upload_url` (`RestClient::upload`, a separate HTTP client that never sends the token, cookies or the super properties, and only to https or loopback), then the message is posted as JSON with `attachments: [{id, filename, uploaded_filename}]`. A file over `State::upload_limit(channel)` (10 MB, 50 MB for Nitro Basic or Classic, 500 MB for Nitro, or the server's boost level: 50 MB at level 2, 100 MB at level 3), or more than 10 files, is refused with `CommandRejected` before any request. `State::message_limit()` is 2000 characters, 4000 with Nitro.

Edits that fail publish `CommandRejected` ("Edit: ...").

## GIFs and stickers

GIF search uses Discord's own endpoints with the web client's parameters, in its order: `GET /gifs/trending?provider=tenor&locale=<l>&media_format=mp4` (categories and the featured GIF), `GET /gifs/search?q=<q>&media_format=mp4&provider=tenor&locale=<l>` and `GET /gifs/suggest?q=<q>&provider=tenor&limit=5&locale=<l>`, as `Command::LoadGifTrending`, `SearchGifs` and `SuggestGifs`. They are API requests, so they carry the session as every other. Results are kept in `State` per query (`gif_trending`, `gif_search`, `gif_suggestions`, the last 20 queries). The picker debounces typing (see "Expression pickers" in `docs/ui.md`). `POST /gifs/select` (`SelectGif`) is only sent when the user turned GIF usage sharing on.

`Command::SendGif` sends the GIF's `url` as the message text and `Command::SendSticker` sends a message with `sticker_ids: [id]` and no text; both go through the same outbox as `SendMessage`: the lock check, a nonce, a pending message (a sticker's shows the sticker), the send pacing and the failed-send handling, so Retry sends the sticker again. Server stickers come from READY and GUILD_CREATE (`guild.stickers`) and GUILD_STICKERS_UPDATE; the standard packs from `GET /sticker-packs` (`LoadStickerPacks`), asked once when the sticker tab first shows.

## Threads, forums and pins

- Active threads come from the gateway (the threads in READY's guilds, THREAD_CREATE, THREAD_UPDATE, THREAD_DELETE and THREAD_LIST_SYNC for subscribed channels); kyoko never asks `GET /guilds/{id}/threads/active`, which user accounts cannot use. Following is THREAD_MEMBER_UPDATE and THREAD_MEMBERS_UPDATE.
- `Command::LoadArchivedThreads { channel_id, more }` and `Command::LoadForumPosts { channel_id, more }` use the web client's thread search, `GET /channels/{id}/threads/search` with `sort_by=last_message_time`, `sort_order=desc`, `limit=25`, `tag_setting=match_some` and `offset` (the threads loaded so far when `more`, nothing more is asked once `has_more` is false). Archived threads add `archived=true`; forum posts leave `archived` out, so one page holds active and archived posts, and its `first_messages` become each post's starter (`State::forum_starter`) without a message request per post. A 202 (index not ready) is asked again after its `retry_after` (2 s without one, kept between 0.3 and 15 s), up to 5 times. A GET, so never paced; it goes through the buckets and the breaker.
- `Command::LoadPins` is `GET /channels/{id}/messages/pins?limit=50` (the web client's route; `items[].message`), and pinning and unpinning are `PUT` and `DELETE /channels/{id}/messages/pins/{message}`. CHANNEL_PINS_UPDATE reloads a pin list that was loaded.
- `Command::CreateThread` is `POST /channels/{id}/messages/{message}/threads` with `name` and `auto_archive_duration`, or `POST /channels/{id}/threads` with `type: 11` as well. It needs Create Public Threads, is held by the channel's safety lock, and is paced as a message send. Discord answers with the thread, which the session stores and selects; the first message in it is an ordinary `SendMessage`.
- `Command::CreateForumPost` goes the way of a send (`task/outbox.rs`): Send Messages, the safety lock, the upload limits, the `before_send` hook for the text, files through the attachment slots and the upload client, then `POST /channels/{forum}/threads` as JSON with `name`, `auto_archive_duration` (the forum's default, 3 days without one), `applied_tags` and `message: { content, attachments: [{ id, filename, uploaded_filename }] }`. It is paced as a message send. The new post is stored and selected; a failure is a `CommandRejected` ("Create post: ...").
- `Command::JoinThread` and `Command::LeaveThread` are `PUT` and `DELETE /channels/{id}/thread-members/@me`; joining is held by the thread's safety lock. `Command::ArchiveThread` is `PATCH /channels/{id}` with `archived`.
- Selecting a forum subscribes the guild and sets the referer, but loads no message history or draft: a forum has none.

## Application commands

`Command::LoadApplicationCommands { channel_id }` fetches the command index the web client uses: `GET /guilds/{id}/application-command-index` in a server, `GET /channels/{id}/application-command-index` in a DM, merged with `GET /users/@me/application-command-index` (user-installed apps). It is fetched once per server or DM per session (again after 60 s if it failed) and kept as `State::command_index(channel_id)` (`Changes::commands`). The composer asks for it when the user types `/` at the start of a message.

`Command::RunApplicationCommand { channel_id, command_id, options }` posts `POST /interactions` as `multipart/form-data` with a `payload_json` part: `type: 2`, `application_id`, `guild_id` (in a server), `channel_id`, the gateway `session_id`, a fresh `nonce`, `analytics_location: "slash_ui"` and `data` (`version`, `id`, `name`, `type: 1`, `options`, `application_command` as the index gave it, `attachments: []`). Interactions are paced like sends and a 429 is not retried. A REST failure, or an `INTERACTION_FAILURE` for the nonce, publishes `CommandRejected`. Only chat-input commands the user typed and submitted run; option autocomplete (interaction type 4) and file options are not supported yet. Locked channels hold commands as they hold sends.

## Message search

`Command::Search { guild_id, channel_id, query }` searches the way the web client does: `GET /guilds/{id}/messages/search` in a server, `GET /channels/{id}/messages/search` in a DM, with `content`, `author_id`, `mentions`, one `has` per kind, `min_id` and `max_id` (after and before), `channel_id` (the `in:` filter), `pinned` and `offset` (left out on the first page). Hits are the `hit: true` entry of each context group; a bad element is skipped.

- One searcher in the session (`crates/session/src/task/search.rs`) holds the current search. A new search waits `search_delay` (300 ms) of quiet first, so a burst of searches is one request with the last query, and only one request is ever in flight; a newer search waits for it and the stale reply is dropped.
- `Command::SearchMore` asks for the next page (offset = hits loaded) while the last page was not empty and fewer than `total_results` are loaded. `Command::CloseSearch` drops the search.
- A 202 reply ("Index not yet available", code 110000) sets `SearchStatus::Indexing` and asks again after its `retry_after` (2 s without one, kept between `search_delay` and 15 s), up to 5 tries, then fails with `SearchFailure::StillIndexing`.
- Any other failure (403, Missing Permissions, a suspended route, a captcha, a 429 the client's own retry did not absorb, 401, network) is `SearchFailure::Refused(SendFailure)` and is never retried; the panel says why.
- Results are not written into the channel caches; opening one in context loads it with `LoadAround`.

## Invites, joining and onboarding

The requests follow the web client, read against discord-userdoccers (docs.discord.food, master `01624d9`, "Invite", "Guild"); the session code is `crates/session/src/task/community.rs`.

- `Command::ResolveInvite { code }`: `GET /invites/{code}?with_counts=true&with_expiration=true`. The preview lands in `State::invite_preview(code)` (server, icon, counts, channel and its type); a 404 marks the code unknown (`State::is_invite_unknown`). Other failures are rejections.
- `Command::AcceptInvite { code, from }`: `POST /invites/{code}` with `{"session_id": "<gateway session id>"}` (`{}` before READY) and `X-Context-Properties` for where the user joined from (`docs/fingerprint.md`). It is a mutation, never retried; a captcha is a rejection saying to finish in the official client and is not sent again. The reply goes to `State::last_join()` (`Join { serial, code, guild_id, onboarding, verification }`, from the server's `GUILD_ONBOARDING` feature and `show_verification_form`), and the session at once loads what the server asks for: `GET /guilds/{id}/member-verification?with_guild=false&invite_code={code}` and `GET /guilds/{id}/onboarding`. The server itself arrives as GUILD_CREATE.
- `Command::LoadMemberVerification { guild_id, invite_code }` and `Command::LoadOnboarding { guild_id }` load them again when the user opens a server whose gate is still closed. Onboarding keeps the prompts in onboarding (`in_onboarding` not false) with their options, and the options the user already chose; a prompt without options, an option without an id or a bad element is dropped.
- `Command::AcceptMemberVerification { guild_id }`: `PUT /guilds/{id}/requests/@me` with `{"version", "form_fields"}`, the loaded form with `response: true` on each rules field. Only forms made of rules (`TERMS`) are sent; any other question is refused before a request ("answer them in the official client").
- `Command::SubmitOnboarding { guild_id, chosen, prompts_seen, options_seen }`: `PUT /guilds/{id}/onboarding-responses` with `{"onboarding_responses": [option ids], "onboarding_prompts_seen": {prompt id: ms}, "onboarding_responses_seen": {option id: ms}}`. Ids that are not options of the loaded onboarding are dropped, and a required prompt without a pick refuses the command before any request. Discord then marks the member (GUILD_MEMBER_UPDATE with COMPLETED_ONBOARDING, or `pending: false` after the rules), which lifts the gate in `docs/locks.md`.
- None of these is ever sent by kyoko on its own: the rules and onboarding requests only follow the user's own confirm in the dialogs.
- `Command::CreateInvite`: `POST /channels/{id}/invites` with `{"max_age", "max_uses", "temporary"}`, nothing else (`target_type` and `unique` absent); Create Instant Invite is checked first. The invite lands in `State::invite_created(channel)`.
- `Command::CreateGuild { name, icon }`: `POST /guilds` with `{"name", "icon", "channels": [], "system_channel_id": null, "guild_template_code": "2TffvPucqHkN"}`, the web client's "Create My Own" template.

Not confirmed against live traffic yet: whether the web client adds `inputValue` to the invite lookup, `flags: 0` to invite creation, and the exact accept body outside guest invites.

## Forwarding

`Command::ForwardMessage { channel_id, message_id, to_channel_id }` posts `POST /channels/{to}/messages` with a fresh `nonce` and `message_reference: {"type": 1, "message_id", "channel_id", "guild_id"}` and no content, through the same send path as messages: the target's channel lock is checked first (a locked target is held with the lock's explanation), Send Messages is checked in a server, and the request is paced as a message send (`send_gap`, `channel_send_gap`), so forwarding to several channels goes out one send apart, never in a burst. A failure is a `CommandRejected` ("Forward: ..."), not retried. No pending row is made for a forward.
