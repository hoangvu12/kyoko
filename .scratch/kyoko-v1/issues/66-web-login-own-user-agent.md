# 66 — Web login presents WebView2's own user agent

**What to build:** The web login window (v1 43) stops overriding its user agent with kyoko's reduced Chrome string and presents WebView2's own, as oxidcord's web login does (`oxidcord-ref/crates/login/src/web_login.rs` sets none). User decision, 2026-10-26: WebView2 always sends its own `Sec-CH-UA` brands (Microsoft Edge WebView2), so a Chrome user agent beside them is the kind of mismatch bot checks (Cloudflare) flag. Everything else from 43 stays: temporary profile, InPrivate, token only from the allowed origin, cookies carried, data cleared.

**Blocked by:** 43 (Web login)

**Status:** ready-for-agent

- [ ] The login window sets no user agent override; the seam test that checked the Chrome string now checks that the page's user agent and its client hints agree (both WebView2's)
- [ ] `docs/sign-in.md` and `docs/fingerprint.md` say the login window is honestly WebView2 and why, and drop the "client hints aren't Chrome's" caveat
- [ ] v1 43's first box wording is updated to match
