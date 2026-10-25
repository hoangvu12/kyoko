# 41 — Multi-account switching

**What to build:** A user can keep several accounts and switch between them, as in oxidcord.

**Blocked by:** 21 (Token login, ToS notice and sign-out)

**Status:** resolved

- [x] The account switcher matches oxidcord in the parity captures
- [x] Each account's token lives in Credential Manager
- [x] Switching reconnects cleanly as the other account

## Answer

Merged to main as `c79bee0`. Docs: `docs/sign-in.md`, `docs/storage.md`, `docs/ui.md`, `docs/testing.md`, `docs/parity.md`.

- Store: one cache database per saved account, `kyoko-<user id>.db` (`Store::for_account`), sharing settings, secrets and the account list with the main store. `kyoko.db` is the device database (installation id, cookies, a layout marker) and the cache for sessions whose user isn't known at start (developer token, the old single token); its first open under the new layout empties the old shared cache once. Signing in as another account no longer wipes anything, so 13's per-account versions can't leak and switching back starts warm. Removing an account deletes its token (`account-<id>` in Credential Manager), entry and database; sign-out deletes the emptied database file.
- Bridge `Flow`: `switch(user_id)` and `add_account()` return a `Handover` that runs off the UI thread, stops the old session completely (gateway closed with 1000, persist queue flushed, unsent paced writes dropped) and makes the target current; `arrived()` starts the next session with the same launch `ClientIdentity`. Only one session ever runs. `accounts()` also lists a signed-in session-only user; `remove_account()` serves Manage Accounts. Adding an account shows the login screen with "Back to your account" (`login.cancel_add`); a missing token asks to sign in again.
- Window: oxidcord's accounts view inside `profile/status_menu.rs` (back, rows with a check on the current one, Add an Account, Manage Accounts / Done with trash buttons); no new overlay layer. A switch closes every dialog, pane, picker and layer, resets the quick switcher's recents and `Link.joins`, empties every view, keeps `reset_connection()` and `FriendsPage::reset()` (which also drops the Add Friend outcome), reloads the rail's open folders from the new account's database and shows a "Switched to <name>" toast. Tray badge and `SetViewing` follow `Flow::generation`; the plugin host re-attaches on session start.
- Mockcord: second token `mock-alt-token`, user `mockalt` (`ALT_USER_ID`), with no servers, DMs or friends and no broadcast events. Partial: routes other than `/users/@me` and READY still answer with the main account's data.
- Tests: `switching_accounts_reconnects_as_the_other_user_with_nothing_shared` (close 1000; same identify properties apart from `installation_id`, which is learned once and kept across accounts; READY of the other user with nothing from the first; separate cached READYs; risky confirmations gone; safety-lock change picked up on switching back), plus token-storage and remove-account bridge tests with the in-memory keyring. Checks re-run by the orchestrator in lane B, all green.
- Parity (port 8496): account-switcher 6.3%, status-menu 6.3% (same baseline, avatar art only). The `account-switcher` step is in `tools/parity/oxidcord-ref.patch`; the shared reference build already has it.
- Deviations: plugin storage stays per plugin, the media cache stays shared (CDN images only; sign-out empties it). No OS-level click-through of a switch (adding needs a pasted token); the seam tests cover the switch and the parity capture drove the view in-process.
- For 42/43/44: hand the token to `Gate::check` → `Flow::checked`; only `Gate::check` saves an account and makes it current, and anything that knows the user id at start goes through `Flow` so the session gets that account's database. Login screens respect `login.adding` and show "Back to your account". Window code reading the database uses `Flow::store()`, not the device store, except for device keys.
