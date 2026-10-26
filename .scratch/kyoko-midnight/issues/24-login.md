# 24 — Login

**What to build:** Restyle the sign-in screens in all their states (email and password with 2FA and captcha, QR scanned and expired, web login, token, unsaved) to the mockups; wording follows `docs/sign-in.md`.

**Blocked by:** 01; v1 21, 42, 43, 44

**Status:** resolved

**Mockups:** `docs/design/mockups/login.html`, `docs/design/mockups/login-*.html`

**Spec:** `docs/design/midnight.md`, Mockups

- [x] Matches the mockups for each state that exists

## Answer

Done with v1 44, merged to main as `3c066ef` (QR column in v1 42, web-login window in v1 43). Every login state with a mockup matches it on Midnight tokens: login-tos 1.6%, login-email 3.2%, login-2fa 1.9%, login-captcha 3.3%, login-unsaved 1.3%, login-token 3.0%, login-qr-scanned 3.1%, login-qr-expired 3.8%, login-web 4.0% (port 8496). Remaining differences: empty fields where the mockups show typed text, QR art, the http mock address, and the expired mockup's "Could not reach Discord." line (kyoko shows a plain expiry). Details in v1 44's Answer.
