# 13 — Versioned read state, persisted client state and late DMs

**What to build:** Unread badges stay correct everywhere, later launches get a smaller initial payload, and DMs that Discord sends after the initial payload appear.

**Blocked by:** 12 (Bounded state caches)

**Status:** resolved

- [x] Read-state and guild-settings versions are tracked and persisted across launches
- [x] IDENTIFY's client_state is built from the persisted versions
- [x] Private channels that arrive after READY are added
- [x] Unread state stays correct for servers that aren't open
- [x] Session tests cover each case

## Answer

Merged to main as `ed94248`. Docs: `docs/client-state.md`.

- `client_state`: fresh `{"guild_versions":{}}`; warm `{"guild_versions":{},"read_state_version":N,"user_guild_settings_version":M}`. Built at every IDENTIFY via `gateway::ClientStateSource` reading `State::client_state()`; the cache loads before the gateway starts. `guild_versions` stays `{}` (guilds can't be merged), `highest_last_message_id`, `private_channels_version` and `user_settings_version` are omitted (no data to back them).
- Persisted: read states in the existing `read_state` table (not encrypted); kv `client_state.read_state_version`; kv `client_state.user_guild_settings` (container with version); kv `session.ready` saved without the two containers. A version on disk is never ahead of its data: a dropped write makes later versions "unknown" for the run; unconfirmed own acks at shutdown forget the versions.
- Merging: a container merges when `partial: true`, kyoko sent a version and it isn't older; otherwise it replaces. MESSAGE_ACK applies only with a newer version (advancing the global read-state version); local reads carry no version; an echo guard restores the local position if an older echo arrives first. Older USER_GUILD_SETTINGS_UPDATE entries are dropped.
- Late DMs: CHANNEL_CREATE without `guild_id` and READY_SUPPLEMENTAL `lazy_private_channels` add the channel with recipients; duplicates ignored. CHANNEL_UNREAD_UPDATE is parsed leniently and only moves `last_message_id`/`last_pin_timestamp` forward.
- A cached READY no longer marks the session Connected (only a live READY with a session id does).
- Sign-out (`session::persist::clear_cache`) clears the snapshot, both versions, the guild-settings container and the read-state table (`Database::replace_read_states(&[])`, `Database::kv_delete`). None of it is keyed per account inside a store, so account switching (41) and 15's wipe must clear it too.
- Mockcord versions both containers, echoes versions in MESSAGE_ACK and answers a known `client_state` with partial containers. Harness `running.restart_after(|mock| ..)`. 6 tests in `tests/session/versions.rs`.
- To confirm on real traffic (59 or a later real run): whether MESSAGE_ACK `version` is global; whether a partial READY appears without CLIENT_STATE_V2; what a fresh web client sends as `client_state` (`api_code_version`, ticket 59); the real CHANNEL_UNREAD_UPDATE shape and rate; `lazy_private_channels` without `private_channels_version`.
- For 22–24: badges from `guild_summary().unread/mentions`, `guild_channel_tree` rows and `dm_rows()`; DM order from `last_message_id`.
