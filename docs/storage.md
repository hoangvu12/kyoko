# Storage

What kyoko keeps on disk, where secrets live, how the local cache stays bounded, and what signing out removes. Code: `crates/store`, with the session side in `crates/session/src/task/storage.rs`.

## Secrets

Three kinds of secret go through one path, `store::secrets`:

- the session token (`session-token`),
- each saved account's token (`account-<user id>`),
- the key of the encrypted local cache (`cache-key`).

By default each lives only in Windows Credential Manager, under the service `kyoko`. The store talks to it through the `store::Keyring` trait: `SystemKeyring` is the real one, and `MemoryKeyring` keeps entries in memory for tests (`Store::open_with(paths, keyring)`). Tests never reach Credential Manager.

Plaintext storage needs the explicit `storage.plaintext_token` setting, off by default. With it on, the secrets are files in the config directory (`token`, `cache-key`, `account-tokens/<user id>.token`) and the keyring is not touched at all, not even to delete. The session publishes `SessionInfo::secrets.plaintext` so the UI can show a warning for as long as the setting is on (tickets 21 and 37). `--mock` and scratch data directories turn the setting on before the first store open.

Turning the setting on or off through `Store::update_settings` moves every secret to the other place: it copies them all first, removes the originals only after every copy succeeded, and rolls the copies back and keeps the old setting if any copy fails.

A keyring error never falls back to a file:

- Saving a token fails with `StoreError::Keyring(reason)`. The token stays in memory for the session, and `SessionInfo::secrets.keyring_error` carries the reason. The reason is a fixed phrase, never the secret. A later successful save clears it.
- If the cache key can't be read, or a new one can't be saved, the store opens the cache in memory for this run and sets `SessionInfo::secrets.cache_in_memory`. Nothing is written to the cache file, so the existing cache on disk is neither read nor dropped, and it is used again once the keyring answers.
- When plaintext is off, a stray plaintext file is never read. A successful keyring save removes one if it exists.

## Pruning

The session prunes the local cache once just after start-up (after the cached snapshot is restored) and then every `interval`, on the blocking pool, never on the session task. The limits are one config, `store::Retention`, passed as `SessionConfig::retention`. The desktop app builds it with `Retention::for_settings`, which takes the media budget from `storage.cache_limit_mb`.

| Field | Default | Meaning |
|---|---|---|
| `messages_per_channel` | 500 | newest rows kept per channel |
| `messages_total` | 50,000 | rows kept over all channels, most recently written first |
| `message_max_age` | 30 days | rows not written for this long are dropped |
| `media_bytes` | 512 MiB | on-disk media cache budget |
| `media_max_age` | 30 days | media files not used for this long are dropped |
| `kept_deleted_per_channel` | 100 | kept deleted messages per channel, most recently deleted first |
| `kept_versions_per_message` | 10 | earlier versions kept per edited message |
| `kept_versions_per_channel` | 500 | earlier versions kept per channel |
| `kept_max_age` | 7 days | kept deleted messages and versions older than this are dropped |
| `interval` | 1 hour | time between runs |

Reasoning:

- Opening a channel reads 50 cached rows (`persist::CACHED_MESSAGES`) and the open channel holds at most 1,000 in memory (`docs/caches.md`). 500 rows per channel lets a channel you scroll back in reopen with history, without the table growing with every page ever loaded.
- 50,000 rows of sealed JSON is roughly 50 to 150 MB on disk, which bounds the database when someone reads hundreds of channels.
- Message age counts from when the row was last written, not from the message timestamp, so a quiet channel you open every week keeps its cache and one you haven't opened in a month loses it. A row is rewritten whenever its page is fetched again.
- The media cache (`<cache>/media/<2 hex>/<sha256>`, written by `media::DiskCache`) uses the file's modified time as its last-use time: `DiskCache::get` touches it on every read. Files past `media_max_age` go first, then the least recently used until the cache fits `media_bytes`.

- Kept deleted messages and edit versions (next section) are what MessageLogger shows. A week covers "what did that say yesterday" without keeping a permanent record of what other people removed. Their age counts from the delete or the edit, not from the last write.

Pruning only deletes store rows and cache files. Memory caps live in `state::CacheLimits` and never delete store rows (ticket 12). Freed database pages are reused by sqlite; the file is compacted on sign-out.

## Kept deleted and edited messages

Kyoko keeps messages other people delete and the earlier versions of edited messages, within the caps above, so the MessageLogger plugin (ticket 52) can show them. The data layer is always on; whether anything is shown is the plugin's choice.

In memory (`state`, see `docs/caches.md`):

- MESSAGE_DELETE and MESSAGE_DELETE_BULK leave the message in its channel window, flagged. `ChannelMessages` also remembers it apart from the window, so a reload or a scrolled page puts it back in place: after every page merge, kept messages between the oldest loaded message (or the start of the channel) and the newest (or anything newer, when the window is at the live end) are spliced back in.
- A deleted message never counts as unread and never places the unread marker.
- Discord leaves a channel's `last_message_id` on a deleted newest message. A window whose newest live message is just below a kept deleted `last_message_id` counts as at the live end, not detached, so the kept message stays in place after a reload.
- A MESSAGE_UPDATE pushes the previous version onto the message's history when the content changed, or when the attachments or embeds changed together with a new `edited_timestamp`. An embed unfurl, which changes embeds without a new edit time, is not a version.
- A deleted message that is only in the referenced-message store (ticket 17) becomes a kept message of its channel when that channel is in memory, so a reply to it can show the kept original.

On disk, two tables added by migration 2, with the message bodies sealed like every other message row:

| Table | Columns | Written when |
|---|---|---|
| `kept_deleted` | `id`, `channel_id`, `deleted_at` (ms), `json` (the message) | a message is deleted. The body comes from memory, or is moved from the `messages` table for channels not in memory. The `messages` row is removed either way, so a cached page never shows a deleted message as live |
| `kept_versions` | `id` (insertion order), `message_id`, `channel_id`, `kept_at` (ms), `json` (`state::MessageVersion`) | an edit replaces a version. For channels not in memory the session applies the edit to the stored row and keeps the old version there |

Opening a channel reads its kept messages and versions (up to the memory caps) before its cached page, so they are in place as soon as the page shows. `Store::prune` enforces the four `kept_*` limits; sign-out wipes both tables with the rest.

Own deletes: a delete kyoko itself sent (`Command::DeleteMessage` / `DeleteMessages`, recognised by a `State::deletions()` entry that is `Queued` or `Deleted` for that message) is not kept. The message leaves the window and the store as before, and any kept versions go with it. The user's own messages deleted anywhere else (another device, a moderator) are kept like anyone else's; a plugin that wants an "ignore self" option compares `message.author` with the current user.

The state API (stable for tickets 26, 45 and 52):

- `MessageView::deleted: Option<DeletedInfo>` (`DeletedInfo { at }`, the local time the delete arrived) and `MessageView::versions: Arc<[MessageVersion]>`, oldest first. `MessageVersion { content, edited_timestamp, replaced_at, attachments, embeds }`: `edited_timestamp` is when that version was written (`None` for the original) and `replaced_at` when the next one replaced it.
- `State::deleted(channel_id, message_id)`, `State::edit_history(channel_id, message_id) -> &[MessageVersion]`, and the same on `ChannelMessages` (`deleted`, `deleted_message`, `edit_history`, `versions_of`).
- `State::shows_deleted_messages()`, set by `Command::ShowDeletedMessages(bool)`, off at every start. While it is off, `channel_rows` leaves deleted messages out and `reply_target` answers `Unavailable` for a kept original; while it is on, deleted rows are included with `deleted` set and `reply_target` answers `ReplyTarget::Deleted(&Message)`. Versions are always on the view; rendering them is the UI's choice.
- `State::deleted_by_me(message_id)` tells the own-delete rule's answer.

Deleted rows still take room in the channel window while hidden, so a channel where most messages get deleted shows fewer live messages until the kept cap trims them.

## Signing out

`Command::Logout` marks the session signed out under the same lock token saves take, so a token refresh that lands later writes nothing. The session then stops, flushes its pending cache writes, and calls `Store::sign_out(user_id)`. That removes:

- the session token and the account's token, from the keyring or the plaintext files,
- the account's entry in `accounts.json`,
- every row of every table in the cache database, which includes cached guilds, channels, users, messages, read state, drafts, the cached READY and recent expressions. Then the database and its WAL are checkpointed and vacuumed,
- the on-disk media cache directory.

The wipe is generic: `Database::wipe_account_data` lists the tables from `sqlite_schema` and empties all of them. A table added by a later migration is account data and is wiped without registering anything, which covers the `kept_deleted` and `kept_versions` tables. Ticket 13's kv keys (`client_state.read_state_version`, `client_state.user_guild_settings`) and its `read_state` rows are account data and go with the rest; the session's sign-out is the only path that clears them. The only rows kept are the device-level `kv` keys in `store::identity::DEVICE_KEYS` (the installation id and the cookie jar) and the cache canary. A new kv key is wiped unless it is added there.

Kept: settings, themes, plugins, logs, the cache key, the installation id and the cookies. The installation id and the cookies describe this browser installation, not the account; Discord's web client keeps both across a logout, and making a fresh device on every sign-out would stand out. Other saved accounts keep their tokens, index entries and their own cache databases (see "Several accounts"). Once the session has let go of it, the signed-out account's emptied database file is deleted too.

## A rejected token

A 401 or gateway close 4004 stops every request and publishes `Disconnected(AuthenticationFailed)` (`docs/rest.md`). The session also marks itself signed out and forgets the rejected token: the session token and the account's token are deleted if they still hold that token. The account entry and the cache stay, so signing back in with the same account is fast and the switcher can still show it. This is not a sign-out: nothing else is wiped. A dead token kept on disk would only be tried again on the next launch.

## Several accounts

Each saved account has its own cache database, `kyoko-<user id>.db` in the data directory, opened with the same cache key by `Store::for_account(user_id)`. That returns a `Store` sharing paths, settings, secrets and the account index with the device store, differing only in `db`. Each account's client-state versions, read states, cached READY, messages, drafts and kept messages live in its own file, so switching never mixes one account's cache into another's, and switching back starts from that account's warm cache. Sessions whose user is unknown at start (a developer token, or the old single session token) use the device database `kyoko.db`.

`kyoko.db` is the device database: the installation id, the cookie jar and the layout marker live there, and `Store::installation_id` and `cookie_jar` always read it, whichever account is signed in. The first open of a store without the marker (`store.layout`) empties the account data in `kyoko.db`, which before ticket 41 was the cache shared by all accounts, then sets the marker.

Removing an account in Manage Accounts deletes its token, index entry and database file. Sign-out wipes the account's database with the same generic `wipe_account_data`, then deletes the file. The on-disk media cache holds only CDN images keyed by URL, is shared by all accounts and is emptied by any sign-out. When the cache key is unavailable, every database, account ones included, is opened in memory for the run.
