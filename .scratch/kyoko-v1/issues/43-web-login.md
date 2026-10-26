# 43 — Web login

**What to build:** A user can sign in on Discord's real login page inside a kyoko WebView2 window, and complete any captcha or 2FA there.

**Blocked by:** 21 (Token login, ToS notice and sign-out)

**Status:** resolved

- [x] The window uses a temporary profile and the same reduced Chrome user agent as the client
- [x] The token is read only from discord.com, by a document-start script, and handed to the app
- [x] Browsing data is cleared afterwards
- [x] Captchas, TOTP, SMS and security keys work on the page

## Comments

From the spike (04): real Discord sets the cookies `__dcfduid`, `__sdcfduid` and `_cfuvid`; the web-login hand-off should carry them into kyoko's shared cookie jar (ticket 06).

## Answer

Merged to main as `fd782d2`. Docs: `docs/sign-in.md`, `docs/fingerprint.md`, `docs/testing.md`, `docs/ui.md`, `docs/parity.md`.

- Page: a WebView2 child window (wry 0.57.1, `default-features = false`, desktop-only dependency) placed by Rust over the page area of a Luau overlay (`shell/login_web.luau`: scrim, title bar, address bar, footer, Cancel) inside the login screen, so the window controls stay above the scrim as in `login-web.html`. Not zgx's built-in WebView2: it can't read HttpOnly cookies and shares one data folder. No root overlay layer (the page is a native child window).
- Bridge `crates/bridge/src/web_login.rs` (`WebSignIn`, UI-free, beside `QrSignIn`): fresh `%TEMP%\kyoko-web-login-*` user-data folder per sign-in, InPrivate, the client's reduced Chrome user agent; a document-start script runs only in the top frame on the allowed origin and posts `localStorage.token`; Rust accepts a token only from that origin; the token goes `Flow::redeem` → `Flow::redeemed` and is never logged. `__dcfduid`, `__sdcfduid`, `_cfuvid` are copied into the shared cookie jar with their lifetimes. On success, cancel, failure, leaving the login screen or quitting: data cleared, page closed, folder deleted (with retries); each launch deletes stray `kyoko-web-login-*` folders. Navigation limited to the allowed origin (other https links open in the system browser); downloads and devtools off.
- The allowed origin is a config value: `https://discord.com`, mockcord's address under `--mock`. Mockcord serves `/login` (sets the three cookies HttpOnly, stores the token in `localStorage`) and `POST /api/v9/auth/login` (outside the auth guard, accepts any non-empty login and password). No test, capture or headless run loads discord.com.
- Removed the inherited helper-process web login and the `tao` and `interprocess` dependencies. The login button reads "Sign in on discord.com". The QR captcha ending offers "Continue on discord.com" (`QrEnd::offers_web_login`, action `login.web_open`).
- Tests: web-login seam tests (token only from the allowed origin, three other origins plus empty and garbage bodies refused, cookies on the next `/users/@me`, user agent, profile folder removed after success, cancel and failure), the extended QR captcha test, 3 load-check screens. A real run on `--mock` (temporary self-login change, reverted) signed in through the window and left no folder. Checks re-run by the orchestrator in lane B: all green, no stray profile folders.
- Mockup parity (port 8496): new `login-web` 5.4% (window, title bar, address bar, footer, Cancel and window controls line up; the card underneath is still the token column, the address bar shows the http mock without a lock), login-token 8.2%.
- Limits: WebView2 sends its own `Sec-CH-UA` brands (Microsoft Edge WebView2); only the user-agent string can be set (in `docs/sign-in.md` and `docs/fingerprint.md`). Captcha, TOTP, SMS and security keys work because it is Discord's own page; not exercised against real Discord.
- Real-Discord confirmation open: that the live page keeps the token in `localStorage.token` (needs a real sign-in; only with the user's go-ahead).
- For 44: extend mockcord's `POST /api/v9/auth/login` with MFA tickets and `/auth/mfa/totp` / `sms` / backup codes; a captcha hands over with `login.web_open` (or `WebSignIn::open`); tokens go to `Flow::redeem` → `Flow::redeemed`. Midnight 24: `login_web.luau` and `login_qr.luau` are Midnight; the left column is still oxidcord's; the overlay shrinks on short windows; the mockup's striped placeholder isn't tokenised, so the page area uses `bg_3`.

User decisions (2026-10-26): the web login window should present WebView2's own user agent, as oxidcord's does (ticket 66), because a Chrome user agent next to WebView2's Edge client hints is a mismatch bot checks look for. No test sign-in on real Discord: oxidcord reads `localStorage.token` the same way, and kyoko captures the storage at document start before Discord removes `window.localStorage`; the user's first real web login is the check.
