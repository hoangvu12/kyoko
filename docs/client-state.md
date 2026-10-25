# Client state and read state

Kyoko remembers enough between launches that Discord can send a smaller READY. IDENTIFY's `client_state` tells Discord which versions of read states and guild settings kyoko already holds. Discord then answers with `partial: true` containers that carry only what changed since.

## What IDENTIFY sends

The gateway asks a `gateway::ClientStateSource` for `protocol::ClientState` each time it identifies, so a re-identify inside one run uses the live versions too. The session's source reads `State::client_state()`.

| Store | `client_state` |
|---|---|
| Fresh, or no trusted version | `{"guild_versions":{},"api_code_version":0}` |
| Warm | `{"guild_versions":{},"read_state_version":N,"user_guild_settings_version":M,"api_code_version":A}` |

`api_code_version` is always sent. It echoes the `api_code_version` of the last READY that carried one, and is `0`, the docs' fresh-client value, before kyoko has seen one. READY's value is decoded leniently: anything other than an integer or a decimal string counts as absent, and an absent value keeps the previous one. It is part of `State::versions()` as `api_code` and is persisted inside the READY snapshot (kv `session.ready`), so the next launch's first IDENTIFY carries it. It describes Discord's API, not cached data, so forgetting the versions at shutdown leaves it alone. Sign-out clears it with the snapshot.

Each of the other versions is sent only when kyoko holds the matching data. The rest are left out on purpose:

- `guild_versions` stays `{}`. Every READY replaces guilds and channels, and kyoko can't merge a guild that Discord sends as partial.
- `highest_last_message_id` and `private_channels_version` are left out. Kyoko doesn't keep the DM list current between READYs: late DMs and DM updates aren't persisted, so it would claim DMs it doesn't have.
- `user_settings_version` is left out, because kyoko doesn't advertise USER_SETTINGS_PROTO.

CLIENT_STATE_V2 (bit 10) stays off.

## Versions

`State::versions()` returns `state::CacheVersions { read_state, user_guild_settings }`.

- READY sets each one from its container's `version`. An unversioned container leaves it `None`.
- MESSAGE_ACK carries a `version`. An ack applies only when its version is newer than the last one seen for that channel. Otherwise it is dropped, so a late or repeated ack can't move read state back. A newer ack applies even when it points at an earlier message, which is how a "mark unread" from another device arrives. The read-state version advances to the highest ack version seen. Acks without a version (kyoko's own local reads) always apply.
- USER_GUILD_SETTINGS_UPDATE is dropped when it is older than the stored entry for that guild. Otherwise it advances the guild-settings version.

## Partial READY

A container merges into the cache when it says `partial: true`, kyoko sent a version for it, and its version isn't older than that. Its entries then replace the cached entry for the same channel or guild, and every other cached entry stays. In any other case the container replaces the cache: a full container, a container without a version, a version mismatch, or a fresh store. Unversioned READYs (capabilities 2 and 3 off) always replace.

## The store

| What | Where |
|---|---|
| Read states | the `read_state` table: channel id, last message id, mention count, last viewed |
| Read-state version | kv `client_state.read_state_version` |
| Guild settings and their version | kv `client_state.user_guild_settings`, one container |
| Guilds, channels, DMs, users | kv `session.ready`, the last READY without its two containers |

After READY the session rewrites the read-state table and the guild-settings container from the merged state. Each MESSAGE_ACK, local read and MESSAGE_CREATE writes the channel's row and then the version, in one job. USER_GUILD_SETTINGS_UPDATE rewrites the guild-settings container. At launch the session loads all of it while it discovers the build number. It applies the cached READY with the cached containers before the gateway connects, so the first IDENTIFY already carries the versions.

A version on disk is never ahead of the data on disk:

- Writes run in order on one queue.
- If a write is dropped (queue full) or fails, every later version write stores "unknown" for the rest of the run.
- If own acks are still unconfirmed at shutdown (pending, in flight, or never echoed), the versions are forgotten. The next launch then gets a full READY.

Sign-out (`Command::Logout`) clears the snapshot, both versions, the guild-settings container and the read-state table.

`ConnectionStatus` is runtime state and is never persisted.

## Own acks and their echoes

Each local read records the message id kyoko expects Discord to echo. If an echo for an earlier message arrives first, the session puts the local read position back. The entry clears once an echo reaches the expected message.

## Late private channels

- CHANNEL_CREATE for a channel without `guild_id` (DM, group DM) adds it to the DM list. Recipients are interned as users. A channel kyoko already has is ignored.
- READY_SUPPLEMENTAL `lazy_private_channels` are added the same way, with `recipient_ids` resolved through READY's `users`.

CHANNEL_UPDATE still replaces a DM.

## Servers that aren't open

Without the passive-guild-update bits, Discord sends CHANNEL_UNREAD_UPDATE for guilds the session isn't subscribed to: `{guild_id, channel_unread_updates: [{id, last_message_id, last_pin_timestamp}]}`. Each entry moves the channel's `last_message_id` and `last_pin_timestamp` forward (never back). The channel's and the guild's unread badges come from those against the read state. Entries that don't decode, and channels kyoko doesn't know, are skipped. MESSAGE_CREATE in unsubscribed guilds works as before.
