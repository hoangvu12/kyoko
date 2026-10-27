# Signing in and out

How kyoko gets from launch to the main view, what the login screen does with a pasted token, and how a rejected token or a sign-out brings the login screen back. Code: `crates/bridge/src/sign_in.rs` (the flow, UI-free and tested at the session seam in `crates/bridge/tests/bridge/sign_in.rs`), `apps/desktop/src/ui/sign_in.rs` (the window side) and `apps/desktop/ui/shell/login.luau` (the screen). The ways in are web login on discord.com, the QR code, the email form and a token. Storage details are in `docs/storage.md`.

## The flow

`bridge::Flow` owns the running `Session` and the screen the window shows:

| Screen | Means |
|---|---|
| `Login(Ready)` | No account; the login screen, nothing to report. |
| `Login(Checking)` | A pasted token is being validated. |
| `Login(Failed(..))` | Validation failed: empty, rejected, network, or refused (captcha or rate limit). |
| `Login(Unsaved(reason))` | The token is valid but Credential Manager refused it. The screen offers to continue for this session only, or to go back (`Flow::discard_unsaved`). |
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

1. A developer token, if any: `KYOKO_TOKEN` against Discord, or mockcord's token with `--mock` and `--mock-server` (unless `--login` is passed, or the capture screen is one of the `login-*` screens). The session runs with `remember_account: false`, so the token is never stored.
2. Otherwise the saved sign-in: the current account's token from `store.accounts`, then the older single session token (`login::persist::load_token`). The session runs with `remember_account: true`.
3. Otherwise the login screen.

Without `--data-dir`, a run against Discord now uses the real data directory (`Paths::discover`), so a sign-in survives a restart. Mock runs keep their scratch directory and plain-text secrets.

## The login screen

The screen follows the Midnight mockups (`login.html`, `login-email.html`, `login-token.html`, `login-2fa.html`, `login-captcha.html`, `login-unsaved.html`, with the QR states and `login-web.html`), on Midnight tokens only, except for which column comes first (below): a card `login_card_width` 784 wide, `space_32` padding, `login_card_gap` 64 between its columns, `radius_lg`, `bg_4` with a `border` line, and the Terms of Service notice under it in `background_feedback_warning` with a `border_feedback_warning` line, `gap` apart. The left column is one of:

- the discord.com column (the default): "Welcome back!", the brand button "Sign in on discord.com" (web login, below), "You'll sign in on Discord's own page, in a Kyoko window.", then two smaller links, "Use email and password instead" and "Sign in with a token", and "Kyoko keeps your token in Windows Credential Manager.";
- the email form, after "Use email and password instead": the email or phone and password fields, "Forgot your password?" (opens web login, where Discord's own reset lives) with "Back to sign-in options" across from it, Log In, then "Sign in with a token" and "Sign in on discord.com" side by side;
- the token form, after "Sign in with a token": the masked Token field with its eye button, Paste token, and "Back to sign-in options";
- the captcha hand-over (below).

The QR column (`login_qr.luau`) sits to the right of all four.

Why discord.com comes first (user decision, 2026-10-27): native email and password login is where other third-party clients' users run into captchas and disabled accounts, so kyoko leads with the two ways that sign in on Discord's own terms, Discord's page and the phone. Nothing is removed; the email form is one click away and unchanged. The Midnight mockups `login.html` and `login-email.html` show the email form as the default; kyoko departs from them there on purpose (`docs/parity.md`).

The column is `bridge::LoginChoice` (`LoginColumn`: `discord`, `email`, `token`), published as `login.mode` and chosen with `login.mode`. It can only change on the login screen, and it goes back to `discord` whenever the login screen leaves (a sign-in, a switch), so a sign-out, an added account or an expired session opens on discord.com again. A failed token or password keeps its form. Tested at the session seam in `crates/bridge/tests/bridge/login_column.rs`, and the links in `apps/desktop/src/ui/scripts/login.rs`. The two-factor step and the unsaved token use the narrow card (`login_card_narrow` 480) without the QR column, with an 80-point badge (`login_badge_size`) above the title.

The notice reads "Kyoko is an unofficial client and is not affiliated with Discord. Using a third-party client with a user account can violate Discord's Terms of Service and can result in account termination. Use it at your own risk." Until "I understand" is ticked, the left column is dimmed to `disabled_opacity_control` and ignores the pointer, and the QR column asks for the notice. Ticking it sets `settings.first_run_acknowledged`, so the notice stays acknowledged across launches and sign-outs.

"Paste token" submits what is typed, or the clipboard when the field is empty. Enter in the field submits too. The field is masked; the eye button shows it. "Sign in on discord.com" opens web login (below); its failures show under the button in whichever column shows it. "Back to your account" shows under every column while adding an account.

`Flow::submit` validates with `login::TokenLogin::validate` (`GET /users/@me` with the web client's headers). Validation never retries: a 429 comes back at once (`max_retry_wait` of zero). Errors show under the field:

| Failure | Shown |
|---|---|
| Empty | Enter or copy a token first. |
| Rejected (401, or a token that cannot be one) | That token was rejected. |
| Network, or any other unexpected answer | Could not reach Discord. |
| Refused (captcha, rate limit, suspended route) | Discord refused the sign-in for now. Finish any check in the official client, then try again later. |

A valid token is saved with `store.accounts.save` and made the current account. Each account has its own cache database, so signing in as another user wipes nothing and never shows the previous account's data. When the keyring fails, nothing is written to disk and the narrow card says "Your token couldn't be saved": "You're signed in as <username>, but Windows Credential Manager refused to store your token.", then "If you continue, Kyoko keeps the token in memory only. You'll need to sign in again the next time you open Kyoko." "Continue for this session only" starts the session with `remember_account: false`; "Back to login" (`Flow::discard_unsaved`) drops the checked token and shows the form again. A new sign-in is always a new `Session`, so safety locks are read again and earlier risky-action confirmations are gone.

Other `Flow` messages (session expired, the session could not start, a missing saved sign-in) show in the column in view: under the discord.com button and its line in the default column, under the password field in the email form, under the token field in the token form.

## Email and password

The email form signs in the way discord.com/login does, with Discord's documented account endpoints (docs.discord.food, Authentication, read 2026-10-26; protocol facts only). Code: `crates/login/src/password_login.rs` (`PasswordLogin`, the requests), `crates/bridge/src/password_login.rs` (`PasswordSignIn`, UI-free, tested at the session seam in `crates/bridge/tests/bridge/password_login.rs`), `apps/desktop/src/ui/sign_in.rs` and `shell/login.luau`.

1. `PasswordSignIn::submit(&Flow, login, password)`, only on an idle login screen with the notice accepted, sends `POST /auth/login` with `{login, password, undelete: false, login_source: null, gift_code_sku_id: null}`. Every request of the form goes through a fresh `RestClient` on the launch identity, with the web client's REST headers, `Referer: https://discord.com/login` (`fingerprint::LOGIN_REFERER`), no `Authorization` and `token_login::without_retries()`: nothing is retried, a 429 included.
2. An answer with a `token` goes straight to step 5. An answer with `mfa: true` and a `ticket` opens the two-factor step with the methods it allows (`totp`, `sms`, `backup`; a `webauthn` challenge is noted as `security_key`). The ticket and `login_instance_id` stay in memory only.
3. The two-factor step starts on the authenticator code when it is allowed, then SMS, then backup codes. "Use a backup code instead", "Use your authenticator app instead" and "Receive auth code from SMS" switch method; the SMS one first sends `POST /auth/mfa/sms/send` with `{ticket}` and shows the redacted phone Discord answers with ("Enter the 6-digit code sent to +*******0085."). An account with only SMS starts on a "Send code" button. "Go back to login" drops the ticket.
4. Log In sends `POST /auth/mfa/<totp|sms|backup>` with `{code, ticket, login_instance_id, login_source: null, gift_code_sku_id: null}`, spaces removed from the code. A wrong code keeps the step and the ticket, so the user can try again.
5. The token goes through `Flow::redeem` and `Flow::redeemed`, as the QR column's does (validate with `GET /users/@me`, save to Credential Manager, make current), so `login.adding`, "Back to your account" and the unsaved card behave the same. A failed check comes back to the email form.

The password, the code, the ticket and the token are never logged or written anywhere; the form's fields are emptied when the main view opens. Leaving the login screen (another sign-in finished) drops the step, and answers that arrive for a dropped step are ignored.

| Answer | Shown |
|---|---|
| 400 form error on `/auth/login` | Discord's own form message, for example "Login or password is invalid.", under the password field, which turns red. Without one, "Login or password is invalid." |
| 400 on a code | Discord's message, for example "Invalid two-factor code", under the code field. Without one, "Invalid two-factor code." |
| Captcha, on any of the requests | The hand-over: "Discord wants a captcha" with "Kyoko can't show Discord's captcha in its own form. Finish signing in on discord.com in a Kyoko window. Kyoko won't retry the sign-in by itself.", a "Continue on discord.com" button (`login.web_open`, web login below) and "Go back to login". Kyoko never shows, solves or retries a captcha. |
| Only a security key | The same hand-over, "Discord wants your security key": "Kyoko's form can't use a security key. Finish signing in on discord.com in a Kyoko window." |
| Rate limit, 403 or a suspended route | Discord refused the sign-in for now. Finish any check in the official client, then try again later. |
| Network, or an answer kyoko can't read | Could not reach Discord. |
| Token rejected by the check | Discord rejected this sign-in. Try again. |
| Empty fields | Enter your email or phone number and your password. / Enter the code first. |

Discord's form messages are shown as plain text, without control characters and at most 300 characters. A new-location check (Discord's emailed or texted login verification) arrives as such a form message; finishing it is the user's, in their email or in the official client.

Not done: the WebAuthn security-key step itself, the `undelete` prompt for disabled accounts and the `required_actions` after a sign-in (`update_password`). The first hands over to discord.com, the second shows Discord's message, the last is ignored.

Under `--mock` the routes are mockcord's (`docs/testing.md`): `mock@kyoko.invalid` signs in at once, `mfa@kyoko.invalid` asks for a code, `captcha@kyoko.invalid` answers with a captcha, all with `mock-password`.

## QR login

The QR column (`shell/login_qr.luau`, styled to the Midnight mockups `login.html`, `login-token.html`, `login-qr-scanned.html` and `login-qr-expired.html`) signs in with Discord's remote-auth gateway, the way discord.com/login does. Code: `crates/login/src/qr/` (the socket, the keys and the ticket exchange), `crates/bridge/src/qr_login.rs` (`QrSignIn`, UI-free, tested at the session seam in `crates/bridge/tests/bridge/qr_login.rs`), `apps/desktop/src/ui/sign_in.rs` and `ui/qr_code.rs` (the window side and the drawn code).

The handshake, from docs.discord.food's Remote Authentication page (read 2026-10-26; protocol facts only):

1. Open `wss://remote-auth-gateway.discord.gg/?v=2` with `Origin: https://discord.com`, the web client's user agent and `Accept-Language`. The server sends `hello` with `heartbeat_interval` and `timeout_ms`.
2. Make a 2048-bit RSA key (off the async threads) and send `init` with the base64 SPKI DER public key. Heartbeat from then on, the first beat jittered; a missed `heartbeat_ack` ends the attempt.
3. `nonce_proof`: decrypt `encrypted_nonce` with RSA-OAEP (SHA-256) and send it back base64url without padding.
4. `pending_remote_init`: the fingerprint must be the base64url SHA-256 of the public key; the code is `https://discord.com/ra/<fingerprint>`.
5. `pending_ticket`: the decrypted payload is `id:discriminator:avatar:username`; the column shows that user's avatar and name and "Check your phone!" before anything is confirmed.
6. `pending_login` with a ticket, or `cancel`. The ticket goes to `POST /users/@me/remote-auth/login` as `{"ticket"}`, with the web client's REST headers (super-properties, client hints, `Origin`, `Referer: https://discord.com/login`) and no `Authorization`; it is never retried. The answer's `encrypted_token` is decrypted with the same key.
7. The token goes through `Flow::redeem`, which is `Gate::check` (validate, save to Credential Manager, make current) like a pasted token, so `login.adding` and "Back to your account" work the same. `Flow::redeemed` hands a failed check back to the QR column instead of the token form.

Nothing of the exchange is logged: not the token, the ticket, the keys or any payload.

The socket is a `wreq` websocket opened by `net::ws::connect` on its own client from `net::client_builder(Destination::Fetch, trust)`, with `net::websocket_emulation()` on the upgrade, so its ClientHello is Chrome's wss hello like the gateway's (`docs/fingerprint.md`, "TLS and HTTP/2 handshake"; `docs/gateway.md`, "Transport"). Inbound frames and messages are capped at 64 KiB, connecting times out after 30 s, and an upgrade answered with anything but 101 fails as a network error that names the HTTP status. The upgrade headers are lowercase in wreq's order, without `Pragma`, `Cache-Control` or `Sec-WebSocket-Extensions` (ticket 74). `QrConfig::trust` is `Trust::Bundled` in the app; the ticket exchange's REST client takes the same trust. `ws://` is accepted on loopback only, for mockcord. Tested in `crates/bridge/tests/bridge/qr_login.rs`: the remote-auth ClientHello against `chrome-156-wss.json`, and a refused upgrade over TLS that names its 404.

`QrSignIn::follow(&Flow)` starts a code whenever the login screen is up and the notice is accepted, and stops it when the screen leaves. A code that ends stays ended until Refresh (`login.qr_refresh`, also "Not me, start over" after a scan) asks for a new one with a new key. Events of an older code are ignored by generation.

| End | Shown |
|---|---|
| Timeout (`timeout_ms`, or close 4003) | The dimmed code, "This code has expired" and Refresh. |
| Cancelled on the phone (`cancel`, or close 1000) | As expired, plus "The login was cancelled on your phone." |
| Captcha on the ticket exchange | As expired, plus "Discord wants a captcha. Kyoko won't solve or retry it by itself: continue on discord.com to finish it on Discord's own page, or refresh to try again." and a "Continue on discord.com" link that opens web login (`QrEnd::offers_web_login`). Kyoko never solves or skips a captcha. |
| Rate limit or suspended route | As expired, plus the token form's "Discord refused the sign-in for now..." |
| Network | As expired, plus "Could not reach Discord." |
| A bad nonce proof, fingerprint or payload (close 4001 or 4002) | As expired, plus "The QR login stopped unexpectedly. Refresh to try again." |

The code is drawn in Rust at the window's scale, `qr_size` less its `space_8` padding, error correction Q (so kyoko's mark can sit in the middle), dark modules in `text_0` on the box's `text_1`. The colours are read when a code arrives, so after a theme switch the next code takes the new colours. The scanning user's avatar goes through the gallery like any other avatar.

Under `--mock` the gateway is mockcord's `/remote-auth` (`Server::remote_auth_url`), which plays the phone (`docs/testing.md`). The signed-in user is mockcord's current user.

## Web login

"Sign in on discord.com" opens Discord's own login page in a WebView2 page inside kyoko's window, so the user finishes any captcha, TOTP, SMS code or security key on Discord's page. Kyoko never fills, solves or automates anything on it. Code: `crates/bridge/src/web_login.rs` (`WebSignIn`, UI-free, tested at the session seam in `crates/bridge/tests/bridge/web_login.rs`), `crates/login/src/web_login.rs` (the token script and Discord's facts), `apps/desktop/src/web_page.rs` (the page, on wry 0.57) and `apps/desktop/ui/shell/login_web.luau` (the window, after the Midnight mockup `login-web.html`).

1. `WebSignIn::open(&Flow)`, only on an idle login screen with the notice accepted, makes a fresh profile folder (`%TEMP%\kyoko-web-login-<pid>-<stamp>-<n>`) and returns a `WebPage`: the login URL, the allowed origin, the folder and the token script. It carries no user agent: the page presents WebView2's own (below).
2. The window draws the overlay in Luau: scrim, title bar with a close dot, an address bar with the page's host and path, the page area, "Browsing data from this window is cleared after you sign in." and Cancel. The page area reports its rectangle as the `web_login` slot, and the window places a WebView2 child window there: InPrivate, on the fresh folder as its user-data folder, devtools off, downloads refused. Top-level navigation stays within the allowed origin; any other link, and any new-window request, opens in the system browser when it is https, never in the page.
3. A document-start script, in the top frame only and only when `location.origin` is the allowed origin, keeps a reference to `localStorage` and polls `token` every 500 ms. Discord's web client keeps the signed-in token there as a JSON string (a protocol fact kyoko has not yet checked against the live page itself). The script posts it with `chrome.webview.postMessage`.
4. `WebSignIn::message(source, body)` takes it only while the page is open and only when the message's source URL has the allowed origin; anything else is ignored. The token is never logged and never written anywhere but Credential Manager through `Gate`.
5. Before the page closes, its cookies for the origin are read through WebView2's cookie manager (HttpOnly ones included), and `__dcfduid`, `__sdcfduid` and `_cfuvid` are carried into the launch's cookie jar (`WebSignIn::carry_cookies`, `rest::CookieJar::adopt`) as host-only cookies of the origin with their lifetimes, so REST continues as the browser that signed in. The session saves the jar as usual.
6. The page's browsing data is cleared, the page closed and its profile folder removed (`Profile::remove` retries for up to 15 seconds while WebView2's browser process lets go; each launch sweeps any `kyoko-web-login-*` left behind). Then the token goes to `Flow::redeem` and `Flow::redeemed`, as the QR column's does, so `login.adding` and "Back to your account" behave the same.

Cancel, the close dot, leaving the login screen (another sign-in finished) and closing the app close the page and remove its folder the same way.

| End | Shown under the button |
|---|---|
| WebView2 Runtime missing, or the page could not be made | "Signing in on discord.com needs the Microsoft Edge WebView2 Runtime, which isn't available. Use a token or the QR code instead." |
| Token rejected | "Discord rejected this sign-in. Try again." |
| Network, refused | As the token form. |

The allowed origin is the origin of `WebLoginConfig::login_url`: `https://discord.com/login` against Discord, mockcord's `/login` under `--mock` and `--mock-server`. No automated test, capture or headless run loads discord.com.

The login window is honestly WebView2, as oxidcord's is: it sets no user agent, so Discord's page sees WebView2's own Edge user agent, and the same browser in its `Sec-CH-UA` header and `navigator.userAgentData.brands` (`Microsoft Edge WebView2`). WebView2 can replace the user agent string but not the client hints, so a Chrome user agent there would sit beside Edge brands, the kind of mismatch bot checks such as Cloudflare's flag. The session that follows still presents the Chrome web client of `docs/fingerprint.md`. `web_page::agent` in `apps/desktop` (run by `cargo test --workspace`) opens a real WebView2 page through `PageHost` on a hidden window against a loopback server and checks that the page's `navigator.userAgent`, its brands and the `User-Agent` and `Sec-CH-UA` headers the server saw agree and are WebView2's.

## Leaving the main view

A 401 or close 4004 at any time publishes `Disconnected(AuthenticationFailed)`. `Flow::observe` sees it on the next change batch, hands the session back to be shut down off the UI thread, and shows the login screen with "Your session expired. Sign in again to continue." The session has already forgotten the rejected token (`docs/storage.md`), so the next launch starts at the login screen too.

Signing out is "Log Out" in the user panel's context menu for now (the panel's menu is ticket 30, account settings ticket 37). `Flow::sign_out` sends `Command::Logout` and returns a `SignOut` job; the window drops the gallery's budget to zero and clears its visible set, then runs the job on the blocking pool. `SignOut::finish` waits for the session to stop and wipe, and removes the current account too if the session never learned who it was. The window then shows the login screen and restores the gallery budget when the next session starts.

Whenever the window leaves the main view it collapses the timeline slot, so a Rust view placed there (ticket 25) never draws over the login screen.

## Several accounts

Switch Accounts in the status menu opens the accounts view (`docs/ui.md`). Accounts are the entries of `store.accounts`: every account signed in with the email form, "Paste token", the QR code or web login is saved there, with its token in Credential Manager under `account-<user id>`.

- Switching: `Flow::switch(user_id)` takes the running session and returns a `Handover`; the screen is `Switching`, and the window closes overlays, settings and the quick switcher's recent list, drops the gallery budget and empties every view at once, so nothing of the old account stays on screen. `Handover::finish` runs on the blocking pool: `Session::shutdown` closes the gateway normally (close 1000) and flushes the persist queue, while unsent paced writes (folder order, collapse, status) are dropped as on quit. It then reads the other account's token and makes that account current. `Flow::arrived` starts the new session with the launch's `ClientIdentity` on that account's own database. Only one session runs at a time. A new session re-reads the safety-lock setting and starts without risky-action confirmations.
- Add an Account: `Flow::add_account` stops the session the same way and shows the login screen with "Back to your account" (`login.cancel_add`), which starts the current account again. A token that validates is saved and becomes current, so the new account starts.
- Manage Accounts removes any account but the signed-in one (`Flow::remove_account`, a `Removal` run on the blocking pool): its token, its index entry and its cache database are deleted.
- When the chosen account's token is gone (forgotten after a 401), the login screen says so (`SignInAgain`) and offers the way back.

Per window, a switch resets what a new session resets (rail, sidebar, DM list, members, timeline, connection banner via `reset_connection`, user panel) plus the community join tracking (`Link.joins`) and the quick switcher's recent targets. The rail's open folders are read from the account's database. The tray badge and `SetViewing` follow the new session: the desktop integration compares `Flow::generation` on every focus poll, re-sends `SetViewing` and redraws the badge. The plugin host is attached to every new session by the `StartSession` closure. Plugin storage and settings are per plugin, not per account: a plugin is a local tool the user installed and its settings apply to whoever is signed in; a plugin that keeps per-user data keys it by the current user. Settings (`settings.json`) are app-wide; nothing in them is account-bound.

## Warnings

`session.storage_warning` carries `SessionInfo::secrets` as a sentence: plain-text token storage on, Credential Manager unavailable, or the cache held in memory. `shell/storage_notice.luau` shows it as a bar under the title bar. Mock runs force plain text on purpose and do not show it.
