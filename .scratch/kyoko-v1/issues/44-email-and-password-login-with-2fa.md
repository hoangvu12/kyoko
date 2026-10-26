# 44 — Email and password login with 2FA

**What to build:** A user can sign in with email and password in a native form and complete TOTP, SMS or backup-code 2FA. If Discord demands a captcha, the user is moved to web login.

**Blocked by:** 43 (Web login)

**Status:** resolved

- [x] The native form and the 2FA step work against mockcord's auth endpoints
- [x] A captcha response hands the user over to web login
- [x] Errors are explained, and nothing is retried automatically

## Answer

Merged to main as `3c066ef` (with Midnight 24's card restyle). Docs: `docs/sign-in.md` ("The login screen" rewritten, "Email and password" with an errors table), `docs/testing.md`, `docs/ui.md`, `docs/parity.md`, `docs/fingerprint.md`, `THIRD_PARTY_NOTICES.md`.

- Login crate `crates/login/src/password_login.rs` (`PasswordLogin`): `POST /auth/login`, `/auth/mfa/sms/send`, `/auth/mfa/{totp,sms,backup}` with shapes from docs.discord.food; a fresh REST client per request with the web client's headers, `Referer: https://discord.com/login`, no Authorization, `without_retries()`. Captcha → `LoginError::Captcha`; a 400 → new `WrongLogin`/`WrongCode` carrying Discord's form message as plain text (control characters removed, at most 300 chars); 429, 403 or suspended → `Refused`. Password, code and ticket in zeroized memory, never logged or on disk.
- Bridge `crates/bridge/src/password_login.rs` (`PasswordSignIn`, UI-free): Form → LoggingIn → Mfa / Texting / Verifying → SigningIn, or a hand-over to web login on a captcha (also on the SMS and code requests) and for accounts whose only 2FA is a security key. Tokens go `Flow::redeem` → `Flow::redeemed`; leaving or going back drops the step and ignores late answers. `Flow::unsaved_user()`, `discard_unsaved()`.
- Mockcord `tools/mockcord/src/password.rs`, password `mock-password`: `mock@kyoko.invalid` (token at once), `mfa@kyoko.invalid` (ticket with TOTP, SMS, backup), `key@kyoko.invalid` (security key only), `captcha@kyoko.invalid` (captcha). Codes: TOTP `123456`, SMS `246810` (after the text), backup `kyoko123`, `000000` → 429. `Control::mfa_tickets_issued()`, `mfa_texts_sent()`.
- Window: actions `login.password_submit`, `login.mfa_verify`, `login.mfa_method`, `login.password_back`, `login.mode`, `login.discard_unsaved`; values `login.password`, `login.mode`, `login.unsaved_name`. `login.luau` restyled to the mockups on Midnight tokens only: card 784 (`space_32` padding, `login_card_gap` 64, `radius_lg`, `bg_4` with a border; 2FA and unsaved use a 480 card), email form with "Sign in with a token" and "Sign in on discord.com", the token form as the token path with "Back to email login", the 2FA step, captcha hand-over, unsaved card, ToS notice. Tokens `login_card_gap`, `login_badge_size`; icons `key`, `globe`, `warning_fill`, `shield_alert`. "Forgot your password?" opens web login.
- Tests: 10 seam tests (password only, TOTP, SMS with redacted phone, backup, wrong password, wrong code keeping the step and ticket, rate limit, security-key-only, back/leave, captcha → exactly one request then `WebSignIn::open` succeeds; headers and exact bodies checked; password and code never on disk; request counts of 1 after a 429). `headless-run` signs in through the email form with TOTP, then with the QR code. Checks re-run by the orchestrator in lane B after rebasing onto 55 (union in `docs/ui.md`): all green (`storage::turning_the_opt_in_on_and_off_moves_the_secrets` flaked once under load).
- Mockup parity (port 8496): login-tos 1.6%, login-email 3.2%, login-2fa 1.9%, login-captcha 3.3%, login-unsaved 1.3%, login-token 3.0%, login-qr-scanned 3.1%, login-qr-expired 3.8% (worker), login-web 4.0%. New screens `login-email`, `login-2fa`, `login-captcha`, `login-unsaved` are mockup-only (`MOCKUP_ONLY`); `login-unsaved` opens the store on a keyring that refuses writes.
- Deviations: `login-token` shows the token form with a rejected token (as its mockup); the unsaved card uses the mockup's wording instead of the keyring error; the 2FA badge uses the existing `lock` icon; the error-code shapes in mockcord (60008, `INVALID_LOGIN`) are from common knowledge (kyoko shows whatever form message Discord sends).
- zgx gotchas (worth adding to `docs/ui.md`): no reactive `opacity` inside a `ui.component` (wrap it); avoid `items = "center"` on columns with wrapping text; `pointer_events` given as a function must return a boolean. In debug builds Luau loads from disk, so `parity mockup --no-build` is enough for Luau-only changes.
- Not exercised against real Discord: new-location email/SMS verification (Discord's message is shown), `undelete`, `required_actions` (`update_password`). Not built: the WebAuthn step (handed to discord.com).
