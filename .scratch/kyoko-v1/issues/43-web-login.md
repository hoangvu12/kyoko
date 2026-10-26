# 43 — Web login

**What to build:** A user can sign in on Discord's real login page inside a kyoko WebView2 window, and complete any captcha or 2FA there.

**Blocked by:** 21 (Token login, ToS notice and sign-out)

**Status:** claimed

- [ ] The window uses a temporary profile and the same reduced Chrome user agent as the client
- [ ] The token is read only from discord.com, by a document-start script, and handed to the app
- [ ] Browsing data is cleared afterwards
- [ ] Captchas, TOTP, SMS and security keys work on the page

## Comments

From the spike (04): real Discord sets the cookies `__dcfduid`, `__sdcfduid` and `_cfuvid`; the web-login hand-off should carry them into kyoko's shared cookie jar (ticket 06).
