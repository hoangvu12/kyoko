# 21 — Token login, ToS notice and sign-out

**What to build:** A user can sign in by pasting a token on a login screen identical to oxidcord's, which includes the Terms of Service notice, and can sign out cleanly.

**Blocked by:** 20 (Theme, fonts, icons and app frame), 15 (Store hardening)

**Status:** ready-for-agent

- [ ] The login screen and its ToS notice match oxidcord in the parity captures
- [ ] A pasted token is validated and stored in Credential Manager, and the app moves on to the main view
- [ ] A 401 at any time returns to this screen
- [ ] Signing out wipes the account's data and returns here
