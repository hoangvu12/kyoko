# 42 — QR login

**What to build:** A user can sign in by scanning a QR code with the Discord mobile app, after confirming the account shown.

**Blocked by:** 21 (Token login, ToS notice and sign-out)

**Status:** claimed

- [ ] The QR screen matches the Midnight mockups (`login.html`, `login-qr-scanned.html`, `login-qr-expired.html`) in mockup captures (Midnight 02; look target switched from oxidcord by the user's decision of 2026-10-26)
- [ ] The remote-auth handshake completes and shows the scanning user before confirmation
- [ ] Cancel, timeout and captcha cases each show a clear message
