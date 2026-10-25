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
| `Main` | A session is running. |
| `SigningOut` | `Command::Logout` was sent and the wipe is running. |

One `ClientIdentity` is made per launch and shared by validation and every session (`docs/fingerprint.md`). The desktop passes `Flow` a `StartSession` closure that builds the `SessionConfig`; tests pass their own.

## Launch

`Flow::launch(developer_token)` starts a session straight away when it can, and otherwise leaves the login screen up:

1. A developer token, if any: `KYOKO_TOKEN` against Discord, or mockcord's token with `--mock` and `--mock-server` (unless `--login` is passed, or the capture screen is `login-tos` or `login-token`). The session runs with `remember_account: false`, so the token is never stored.
2. Otherwise the saved sign-in: the current account's token from `store.accounts`, then the older single session token (`login::persist::load_token`). The session runs with `remember_account: true`.
3. Otherwise the login screen.

Without `--data-dir`, a run against Discord now uses the real data directory (`Paths::discover`), so a sign-in survives a restart. Mock runs keep their scratch directory and plain-text secrets.

## The login screen

The screen is oxidcord's: a card with the token column and the QR column, and the Terms of Service notice under it. The notice reads "Kyoko is an unofficial client and is not affiliated with Discord. Using a third-party client with a user account can violate Discord's Terms of Service and can result in account termination. Use it at your own risk." Until "I understand" is ticked, the token field and both buttons are disabled and the QR column asks for the notice. Ticking it sets `settings.first_run_acknowledged`, so the notice stays acknowledged across launches and sign-outs.

"Paste token" submits what is typed, or the clipboard when the field is empty. Enter in the field submits too. The field is masked; the eye button shows it. The QR column and "Web login" say "coming soon" until tickets 42 and 43.

`Flow::submit` validates with `login::TokenLogin::validate` (`GET /users/@me` with the web client's headers). Validation never retries: a 429 comes back at once (`max_retry_wait` of zero). Errors:

| Failure | Shown |
|---|---|
| Empty | Enter or copy a token first. |
| Rejected (401, or a token that cannot be one) | That token was rejected. |
| Network, or any other unexpected answer | Could not reach Discord. |
| Refused (captcha, rate limit, suspended route) | Discord refused the sign-in for now. Finish any check in the official client, then try again later. |

A valid token is saved with `store.accounts.save` and made the current account. If the previous current account was another user, the shared cache is wiped first so their data never shows under the new account. When the keyring fails, nothing is written to disk; the screen shows the error and a "Continue for this session only" button, which starts the session with `remember_account: false`. A new sign-in is always a new `Session`, so safety locks are read again and earlier risky-action confirmations are gone.

## Leaving the main view

A 401 or close 4004 at any time publishes `Disconnected(AuthenticationFailed)`. `Flow::observe` sees it on the next change batch, hands the session back to be shut down off the UI thread, and shows the login screen with "Your session expired. Sign in again to continue." The session has already forgotten the rejected token (`docs/storage.md`), so the next launch starts at the login screen too.

Signing out is "Log Out" in the user panel's context menu for now (the panel's menu is ticket 30, account settings ticket 37). `Flow::sign_out` sends `Command::Logout` and returns a `SignOut` job; the window drops the gallery's budget to zero and clears its visible set, then runs the job on the blocking pool. `SignOut::finish` waits for the session to stop and wipe, and removes the current account too if the session never learned who it was. The window then shows the login screen and restores the gallery budget when the next session starts.

Whenever the window leaves the main view it collapses the timeline slot, so a Rust view placed there (ticket 25) never draws over the login screen.

## Warnings

`session.storage_warning` carries `SessionInfo::secrets` as a sentence: plain-text token storage on, Credential Manager unavailable, or the cache held in memory. `shell/storage_notice.luau` shows it as a bar under the title bar. Mock runs force plain text on purpose and do not show it.
