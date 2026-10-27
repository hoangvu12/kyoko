# 68 — The login screen puts discord.com and the QR code first

**What to build:** The big login card leads with the two ways that sign in on Discord's own terms: "Sign in on discord.com" (the WebView2 web login of v1 43/66) and the QR column (v1 42). The email and password form moves behind a smaller "Use email and password instead" link, one click away; nothing is removed, and "Sign in with a token" stays. User decision, 2026-10-27: native password login is where other third-party clients' users hit captchas and disabled accounts, so kyoko should steer people to discord.com or the phone. This overrides the Midnight mockups (`login.html`, `login-email.html`), which show the email form as the default; follow the mockups' tokens, spacing and type for everything else.

**Blocked by:** none

**Status:** resolved

- [x] The default left column of the big card offers "Sign in on discord.com" as its main (brand) button, with a short line that says the sign-in happens on Discord's own page, next to the QR column; the email form is no longer the default
- [x] A smaller "Use email and password instead" link opens today's email form unchanged (fields, Forgot your password?, Log In, two-factor, captcha hand-over); the email form gets a way back to the default column; "Sign in with a token" is reachable from the default column and the token form's back link returns there
- [x] Every existing ending still lands where it did: web login failures under the discord.com button, the captcha hand-over's "Continue on discord.com", QR captcha's "Continue on discord.com", "Back to your account" while adding an account, the unsaved card, the session-expired and sign-in-again messages, the ToS notice dimming
- [x] Bridge or desktop seam tests (and the Luau load check screens in `apps/desktop/src/ui/scripts.rs`) cover the new default column, the email link and the way back
- [x] Captures: `login-tos` (default), `login-email` (after the link), `login-token`, `login-web`, `login-2fa`, `login-captcha`, `login-unsaved`, `login-qr-scanned`, `login-qr-expired` checked by eye on the lane's port; differing-pixel shares recorded (`login-tos`/`login-email` will differ from their mockups by design)
- [x] `docs/sign-in.md` ("The login screen") and `docs/ui.md` describe the new order and why; `docs/parity.md` notes the deliberate difference from `login.html`

## Answer

Merged to main as `7e6705f`. Docs: `docs/sign-in.md` ("The login screen": new order, why, `LoginChoice`, where Flow messages show), `docs/ui.md`, `docs/parity.md` (deliberate difference from `login.html`).

- Default column (`shell/login.luau`): "Welcome back!", the brand button "Sign in on discord.com", the line "You'll sign in on Discord's own page, in a Kyoko window.", then two small links "Use email and password instead" and "Sign in with a token", and the Credential Manager note. Web login failures and Flow messages (session expired, sign in again, unavailable) show under the button and its line. The ToS notice dims it as before.
- The email form is unchanged except "Back to sign-in options" on the "Forgot your password?" row (keeps the mockup height); the token form's back link has the same text and returns to the default column.
- `login.mode` is `discord` (default), `email` or `token`, held by `bridge::LoginChoice`/`LoginColumn` (`crates/bridge/src/login_column.rs`): it changes only while the login screen is up and resets to `discord` whenever the screen leaves, so a sign-out, an added account or an expired session opens on discord.com, while a rejected token or password stays on its form.
- Tests: bridge seam `login_column::the_login_screen_opens_on_discord_com_and_returns_there_after_a_session`; desktop `ui::scripts::login::the_login_card_leads_with_discord_com_and_keeps_email_one_click_away` (clicks by id: `login_discord`, `login_email_link`, `login_token_link`, `login_email_back`, `login_token_back`); new load-check samples in `login_forms`.
- Captures: no `capture.rs` change; `drive_capture` picks the email column before `login-email`, `login-2fa`, `login-captcha`, `login-unsaved`. Mockup parity (port 8496), by design higher where the mockup shows the email form: login-tos 14.1%, login-qr-scanned 14.3%, login-qr-expired 17.7%; otherwise login-email 3.3%, login-web 5.5%, login-token 3.1%, login-captcha 3.3%, login-2fa 1.9%, login-unsaved 1.3%.
- Deviation: in the default column "Sign in with a token" is a small link, not the half-width button it was, so it doesn't outrank the email link.
- Checks re-run by the orchestrator in lane B: all green.