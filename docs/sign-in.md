# Signing in and out

How kyoko gets from launch to the main view, what the login screen does with a pasted token, and how a rejected token or a sign-out brings the login screen back. Code: `crates/bridge/src/sign_in.rs` (the flow, UI-free and tested at the session seam in `crates/bridge/tests/bridge/sign_in.rs`), `apps/desktop/src/ui/sign_in.rs` (the window side) and `apps/desktop/ui/shell/login.luau` (the screen). Storage details are in `docs/storage.md`.

## The flow

`bridge::Flow` owns the running `Session` and the screen the window shows:

| Screen | Means |
|---|---|
| `Login(Ready)` | No account; the login screen, nothing to report. |
| `Login(Checking)` | A pasted token is being validated. |
| `Login(Failed(..))` | Validation failed: empty, rejected, network, or refused (captcha or rate limit). |
| `Login(Unsaved(reason))` | The token is valid but Credential Manager refused it. The screen offers to continue for this session only. |
| `Login(Expired)` | Discord rejected the token mid-session (401 or close 4004). |
| `Login(SignedOut)` | Back from a sign-out. |
| `Login(Unavailable)` | The session could not start. |
| `Login(SignInAgain)` | A switch found no saved token for the chosen account. |
| `Main` | A session is running. |
| `SigningOut` | `Command::Logout` was sent and the wipe is running. |
| `Switching` | The old session is stopping before another account starts, or before the login screen opens to add one. |

One `ClientIdentity` is made per launch and shared by validation and every session (`docs/fingerprint.md`). The desktop passes `Flow` a `StartSession` closure that builds the `SessionConfig` from a `Start { token, remember, store }`; tests pass their own. `store` is the account's own cache (`Store::for_account`, see `docs/storage.md`) when the account is known, otherwise the device store.

## Launch

`Flow::launch(developer_token)` starts a session straight away when it can, and otherwise leaves the login screen up:

1. A developer token, if any: `KYOKO_TOKEN` against Discord, or mockcord's token with `--mock` and `--mock-server` (unless `--login` is passed, or the capture screen is `login-tos` or `login-token`). The session runs with `remember_account: false`, so the token is never stored.
2. Otherwise the saved sign-in: the current account's token from `store.accounts`, then the older single session token (`login::persist::load_token`). The session runs with `remember_account: true`.
3. Otherwise the login screen.

Without `--data-dir`, a run against Discord now uses the real data directory (`Paths::discover`), so a sign-in survives a restart. Mock runs keep their scratch directory and plain-text secrets.

## The login screen

The screen is oxidcord's: a card with the token column and the QR column, and the Terms of Service notice under it. The notice reads "Kyoko is an unofficial client and is not affiliated with Discord. Using a third-party client with a user account can violate Discord's Terms of Service and can result in account termination. Use it at your own risk." Until "I understand" is ticked, the token field and both buttons are disabled and the QR column asks for the notice. Ticking it sets `settings.first_run_acknowledged`, so the notice stays acknowledged across launches and sign-outs.

"Paste token" submits what is typed, or the clipboard when the field is empty. Enter in the field submits too. The field is masked; the eye button shows it. "Web login" says "coming soon" until ticket 43.

`Flow::submit` validates with `login::TokenLogin::validate` (`GET /users/@me` with the web client's headers). Validation never retries: a 429 comes back at once (`max_retry_wait` of zero). Errors:

| Failure | Shown |
|---|---|
| Empty | Enter or copy a token first. |
| Rejected (401, or a token that cannot be one) | That token was rejected. |
| Network, or any other unexpected answer | Could not reach Discord. |
| Refused (captcha, rate limit, suspended route) | Discord refused the sign-in for now. Finish any check in the official client, then try again later. |

A valid token is saved with `store.accounts.save` and made the current account. Each account has its own cache database, so signing in as another user wipes nothing and never shows the previous account's data. When the keyring fails, nothing is written to disk; the screen shows the error and a "Continue for this session only" button, which starts the session with `remember_account: false`. A new sign-in is always a new `Session`, so safety locks are read again and earlier risky-action confirmations are gone.

## QR login

The QR column (`shell/login_qr.luau`, styled to the Midnight mockups `login.html`, `login-token.html`, `login-qr-scanned.html` and `login-qr-expired.html`) signs in with Discord's remote-auth gateway, the way discord.com/login does. Code: `crates/login/src/qr/` (the socket, the keys and the ticket exchange), `crates/bridge/src/qr_login.rs` (`QrSignIn`, UI-free, tested at the session seam in `crates/bridge/tests/bridge/qr_login.rs`), `apps/desktop/src/ui/sign_in.rs` and `ui/qr_code.rs` (the window side and the drawn code).

The handshake, from docs.discord.food's Remote Authentication page (read 2026-10-26; protocol facts only):

1. Open `wss://remote-auth-gateway.discord.gg/?v=2` with `Origin: https://discord.com` and the web client's user agent. The server sends `hello` with `heartbeat_interval` and `timeout_ms`.
2. Make a 2048-bit RSA key (off the async threads) and send `init` with the base64 SPKI DER public key. Heartbeat from then on, the first beat jittered; a missed `heartbeat_ack` ends the attempt.
3. `nonce_proof`: decrypt `encrypted_nonce` with RSA-OAEP (SHA-256) and send it back base64url without padding.
4. `pending_remote_init`: the fingerprint must be the base64url SHA-256 of the public key; the code is `https://discord.com/ra/<fingerprint>`.
5. `pending_ticket`: the decrypted payload is `id:discriminator:avatar:username`; the column shows that user's avatar and name and "Check your phone!" before anything is confirmed.
6. `pending_login` with a ticket, or `cancel`. The ticket goes to `POST /users/@me/remote-auth/login` as `{"ticket"}`, with the web client's REST headers (super-properties, client hints, `Origin`, `Referer: https://discord.com/login`) and no `Authorization`; it is never retried. The answer's `encrypted_token` is decrypted with the same key.
7. The token goes through `Flow::redeem`, which is `Gate::check` (validate, save to Credential Manager, make current) like a pasted token, so `login.adding` and "Back to your account" work the same. `Flow::redeemed` hands a failed check back to the QR column instead of the token form.

Nothing of the exchange is logged: not the token, the ticket, the keys or any payload.

`QrSignIn::follow(&Flow)` starts a code whenever the login screen is up and the notice is accepted, and stops it when the screen leaves. A code that ends stays ended until Refresh (`login.qr_refresh`, also "Not me, start over" after a scan) asks for a new one with a new key. Events of an older code are ignored by generation.

| End | Shown |
|---|---|
| Timeout (`timeout_ms`, or close 4003) | The dimmed code, "This code has expired" and Refresh. |
| Cancelled on the phone (`cancel`, or close 1000) | As expired, plus "The login was cancelled on your phone." |
| Captcha on the ticket exchange | As expired, plus "Discord wants a captcha. Kyoko can't show Discord's captcha and won't retry by itself. Finish any check in the official client, then refresh." Kyoko never solves or skips a captcha. |
| Rate limit or suspended route | As expired, plus the token form's "Discord refused the sign-in for now..." |
| Network | As expired, plus "Could not reach Discord." |
| A bad nonce proof, fingerprint or payload (close 4001 or 4002) | As expired, plus "The QR login stopped unexpectedly. Refresh to try again." |

The code is drawn in Rust at the window's scale, `qr_size` less its `space_8` padding, error correction Q (so kyoko's mark can sit in the middle), dark modules in `text_0` on the box's `text_1`. The colours are read when a code arrives, so after a theme switch the next code takes the new colours. The scanning user's avatar goes through the gallery like any other avatar.

Under `--mock` the gateway is mockcord's `/remote-auth` (`Server::remote_auth_url`), which plays the phone (`docs/testing.md`). The signed-in user is mockcord's current user.

## Leaving the main view

A 401 or close 4004 at any time publishes `Disconnected(AuthenticationFailed)`. `Flow::observe` sees it on the next change batch, hands the session back to be shut down off the UI thread, and shows the login screen with "Your session expired. Sign in again to continue." The session has already forgotten the rejected token (`docs/storage.md`), so the next launch starts at the login screen too.

Signing out is "Log Out" in the user panel's context menu for now (the panel's menu is ticket 30, account settings ticket 37). `Flow::sign_out` sends `Command::Logout` and returns a `SignOut` job; the window drops the gallery's budget to zero and clears its visible set, then runs the job on the blocking pool. `SignOut::finish` waits for the session to stop and wipe, and removes the current account too if the session never learned who it was. The window then shows the login screen and restores the gallery budget when the next session starts.

Whenever the window leaves the main view it collapses the timeline slot, so a Rust view placed there (ticket 25) never draws over the login screen.

## Several accounts

Switch Accounts in the status menu opens the accounts view (`docs/ui.md`). Accounts are the entries of `store.accounts`: every account signed in with "Paste token" or the QR code (and later web login) is saved there, with its token in Credential Manager under `account-<user id>`.

- Switching: `Flow::switch(user_id)` takes the running session and returns a `Handover`; the screen is `Switching`, and the window closes overlays, settings and the quick switcher's recent list, drops the gallery budget and empties every view at once, so nothing of the old account stays on screen. `Handover::finish` runs on the blocking pool: `Session::shutdown` closes the gateway normally (close 1000) and flushes the persist queue, while unsent paced writes (folder order, collapse, status) are dropped as on quit. It then reads the other account's token and makes that account current. `Flow::arrived` starts the new session with the launch's `ClientIdentity` on that account's own database. Only one session runs at a time. A new session re-reads the safety-lock setting and starts without risky-action confirmations.
- Add an Account: `Flow::add_account` stops the session the same way and shows the login screen with "Back to your account" (`login.cancel_add`), which starts the current account again. A token that validates is saved and becomes current, so the new account starts.
- Manage Accounts removes any account but the signed-in one (`Flow::remove_account`, a `Removal` run on the blocking pool): its token, its index entry and its cache database are deleted.
- When the chosen account's token is gone (forgotten after a 401), the login screen says so (`SignInAgain`) and offers the way back.

Per window, a switch resets what a new session resets (rail, sidebar, DM list, members, timeline, connection banner via `reset_connection`, user panel) plus the community join tracking (`Link.joins`) and the quick switcher's recent targets. The rail's open folders are read from the account's database. The tray badge and `SetViewing` follow the new session: the desktop integration compares `Flow::generation` on every focus poll, re-sends `SetViewing` and redraws the badge. The plugin host is attached to every new session by the `StartSession` closure. Plugin storage and settings are per plugin, not per account: a plugin is a local tool the user installed and its settings apply to whoever is signed in; a plugin that keeps per-user data keys it by the current user. Settings (`settings.json`) are app-wide; nothing in them is account-bound.

## Warnings

`session.storage_warning` carries `SessionInfo::secrets` as a sentence: plain-text token storage on, Credential Manager unavailable, or the cache held in memory. `shell/storage_notice.luau` shows it as a bar under the title bar. Mock runs force plain text on purpose and do not show it.
