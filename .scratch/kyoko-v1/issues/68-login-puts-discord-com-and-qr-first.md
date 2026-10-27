# 68 — The login screen puts discord.com and the QR code first

**What to build:** The big login card leads with the two ways that sign in on Discord's own terms: "Sign in on discord.com" (the WebView2 web login of v1 43/66) and the QR column (v1 42). The email and password form moves behind a smaller "Use email and password instead" link, one click away; nothing is removed, and "Sign in with a token" stays. User decision, 2026-10-27: native password login is where other third-party clients' users hit captchas and disabled accounts, so kyoko should steer people to discord.com or the phone. This overrides the Midnight mockups (`login.html`, `login-email.html`), which show the email form as the default; follow the mockups' tokens, spacing and type for everything else.

**Blocked by:** none

**Status:** claimed

- [ ] The default left column of the big card offers "Sign in on discord.com" as its main (brand) button, with a short line that says the sign-in happens on Discord's own page, next to the QR column; the email form is no longer the default
- [ ] A smaller "Use email and password instead" link opens today's email form unchanged (fields, Forgot your password?, Log In, two-factor, captcha hand-over); the email form gets a way back to the default column; "Sign in with a token" is reachable from the default column and the token form's back link returns there
- [ ] Every existing ending still lands where it did: web login failures under the discord.com button, the captcha hand-over's "Continue on discord.com", QR captcha's "Continue on discord.com", "Back to your account" while adding an account, the unsaved card, the session-expired and sign-in-again messages, the ToS notice dimming
- [ ] Bridge or desktop seam tests (and the Luau load check screens in `apps/desktop/src/ui/scripts.rs`) cover the new default column, the email link and the way back
- [ ] Captures: `login-tos` (default), `login-email` (after the link), `login-token`, `login-web`, `login-2fa`, `login-captcha`, `login-unsaved`, `login-qr-scanned`, `login-qr-expired` checked by eye on the lane's port; differing-pixel shares recorded (`login-tos`/`login-email` will differ from their mockups by design)
- [ ] `docs/sign-in.md` ("The login screen") and `docs/ui.md` describe the new order and why; `docs/parity.md` notes the deliberate difference from `login.html`
