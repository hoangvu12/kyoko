# 44 — Email and password login with 2FA

**What to build:** A user can sign in with email and password in a native form and complete TOTP, SMS or backup-code 2FA. If Discord demands a captcha, the user is moved to web login.

**Blocked by:** 43 (Web login)

**Status:** ready-for-agent

- [ ] The native form and the 2FA step work against mockcord's auth endpoints
- [ ] A captcha response hands the user over to web login
- [ ] Errors are explained, and nothing is retried automatically
