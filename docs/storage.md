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
| `interval` | 1 hour | time between runs |

Reasoning:

- Opening a channel reads 50 cached rows (`persist::CACHED_MESSAGES`) and the open channel holds at most 1,000 in memory (`docs/caches.md`). 500 rows per channel lets a channel you scroll back in reopen with history, without the table growing with every page ever loaded.
- 50,000 rows of sealed JSON is roughly 50 to 150 MB on disk, which bounds the database when someone reads hundreds of channels.
- Message age counts from when the row was last written, not from the message timestamp, so a quiet channel you open every week keeps its cache and one you haven't opened in a month loses it. A row is rewritten whenever its page is fetched again.
- The media cache (`<cache>/media/<2 hex>/<sha256>`, written by `media::DiskCache`) uses the file's modified time as its last-use time: `DiskCache::get` touches it on every read. Files past `media_max_age` go first, then the least recently used until the cache fits `media_bytes`.

Pruning only deletes store rows and cache files. Memory caps live in `state::CacheLimits` and never delete store rows (ticket 12). Freed database pages are reused by sqlite; the file is compacted on sign-out.

## Signing out

`Command::Logout` marks the session signed out under the same lock token saves take, so a token refresh that lands later writes nothing. The session then stops, flushes its pending cache writes, and calls `Store::sign_out(user_id)`. That removes:

- the session token and the account's token, from the keyring or the plaintext files,
- the account's entry in `accounts.json`,
- every row of every table in the cache database, which includes cached guilds, channels, users, messages, read state, drafts, the cached READY and recent expressions. Then the database and its WAL are checkpointed and vacuumed,
- the on-disk media cache directory.

The wipe is generic: `Database::wipe_account_data` lists the tables from `sqlite_schema` and empties all of them. A table added by a later migration is account data and is wiped without registering anything, which covers ticket 16's kept deleted and edited messages. Ticket 13's kv keys (`client_state.read_state_version`, `client_state.user_guild_settings`) and its `read_state` rows are account data and go with the rest; the session's sign-out is the only path that clears them. The only rows kept are the device-level `kv` keys in `store::identity::DEVICE_KEYS` (the installation id and the cookie jar) and the cache canary. A new kv key is wiped unless it is added there.

Kept: settings, themes, plugins, logs, the cache key, the installation id and the cookies. The installation id and the cookies describe this browser installation, not the account; Discord's web client keeps both across a logout, and making a fresh device on every sign-out would stand out. Other saved accounts keep their tokens and index entries, but the shared cache is emptied (see "Several accounts").

## A rejected token

A 401 or gateway close 4004 stops every request and publishes `Disconnected(AuthenticationFailed)` (`docs/rest.md`). The session also marks itself signed out and forgets the rejected token: the session token and the account's token are deleted if they still hold that token. The account entry and the cache stay, so signing back in with the same account is fast and the switcher can still show it. This is not a sign-out: nothing else is wiped. A dead token kept on disk would only be tried again on the next launch.

## Several accounts

The cache database is shared by every account. Until ticket 41, signing out of one account empties the whole cache. When accounts become switchable, give each account its own database file (for example `kyoko-<user id>.db`) so `sign_out` can delete one file and switching opens another; `wipe_account_data` already treats the whole database as one account's data.
