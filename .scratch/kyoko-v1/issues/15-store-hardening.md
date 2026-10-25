# 15 — Store hardening

**What to build:** The token never lands in plaintext unless the user opts in, and on-disk data doesn't grow forever.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** resolved

- [x] The token lives only in Windows Credential Manager. Plaintext storage needs an explicit setting and shows a warning
- [x] A keyring error never silently falls back to a plaintext file
- [x] The local message table and the media cache are pruned by size and age
- [x] Signing out wipes the token, cached messages and kept deleted messages
- [x] Session tests cover storage, the opt-in path and pruning

## Answer

Merged to main as `a19b174..f2a3421`. Docs: `docs/storage.md`.

- Secrets go through `store::Keyring` (`SystemKeyring`, service `kyoko`; `MemoryKeyring` for tests, can simulate outages). `Store::open` uses the real one, `Store::open_with(paths, keyring)` another. Plaintext only with `storage.plaintext_token` (default off); then the keyring is never touched. State: `SessionInfo::secrets { plaintext, keyring_error, cache_in_memory }` for the UI's warnings. A keyring error never writes a file; the token stays in memory. Toggling the setting moves all secrets (copy, then delete, else roll back). `login::save_token` returns the keyring error.
- Pruning (`store::Retention`, `SessionConfig::retention`): 500 messages per channel, 50,000 total, 30 days since last write; media cache 512 MiB (desktop takes `cache_limit_mb`) and 30 days LRU; every hour, on the blocking pool. The media cache stays at `paths.cache_dir()/media` through `DiskCache` (mtime = last use); `DiskCache::evict_to` is redundant.
- Sign-out: `Command::Logout` marks signed out; the session flushes and closes the persist queue; then `Store::sign_out(user_id)` removes tokens and the account entry, `Database::wipe_account_data` empties every table in `sqlite_schema`, vacuums, and deletes the media cache. Every kv key is wiped unless listed in `store::identity::DEVICE_KEYS` (installation id, cookie jar). 13's `clear_cache` is gone; this is the single wipe path, covering 13's kv keys and `read_state`. New tables (16) are wiped automatically.
- A rejected token (401 / close 4004) is forgotten (only if the slots still hold it), the session is signed out, the account entry and cache stay.
- Shutdown stall: not sqlite or the persist thread. Fire-and-forget `accounts.json`/`settings.json` writes outlive the session and file I/O under load took 0.3–30 s (antivirus/disk); `write_atomic` now uses a unique temp file (two writers collided on `<name>.tmp`). What remains is environment I/O.
- 10 tests in `tests/session/storage.rs`; harness defaults are now production (plaintext off, in-memory keyring).
- For 16: add kept-message caps to `Retention` and prune them in `Store::prune`. For 18: sign-out deletes the media dir, so stop the media service first. For 21: route to login on `Disconnected(AuthenticationFailed)`, show warnings from `SessionInfo::secrets`. For 41: the cache database is shared by all accounts; use one database per account (e.g. `kyoko-<user id>.db`).
