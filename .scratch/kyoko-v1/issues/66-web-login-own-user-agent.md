# 66 — Web login presents WebView2's own user agent

**What to build:** The web login window (v1 43) stops overriding its user agent with kyoko's reduced Chrome string and presents WebView2's own, as oxidcord's web login does (`oxidcord-ref/crates/login/src/web_login.rs` sets none). User decision, 2026-10-26: WebView2 always sends its own `Sec-CH-UA` brands (Microsoft Edge WebView2), so a Chrome user agent beside them is the kind of mismatch bot checks (Cloudflare) flag. Everything else from 43 stays: temporary profile, InPrivate, token only from the allowed origin, cookies carried, data cleared.

**Blocked by:** 43 (Web login)

**Status:** resolved

- [x] The login window sets no user agent override; the seam test that checked the Chrome string now checks that the page's user agent and its client hints agree (both WebView2's)
- [x] `docs/sign-in.md` and `docs/fingerprint.md` say the login window is honestly WebView2 and why, and drop the "client hints aren't Chrome's" caveat
- [x] v1 43's first box wording is updated to match

## Answer

Merged to main as `c1ba933`. Docs: `docs/sign-in.md`, `docs/fingerprint.md` (the login window is honestly WebView2: WebView2 lets an app change the user agent but not the client hints, so a Chrome UA beside Edge WebView2 brands is a mismatch bot checks flag; the session after it still presents the Chrome web client), `docs/testing.md`.

- The `user_agent` field is gone from `WebPage` (`crates/bridge/src/web_login.rs`) and `.with_user_agent(..)` from `apps/desktop/src/web_page.rs`. Everything else from 43 unchanged (temporary profile, InPrivate, token only from the allowed origin, cookies carried, data cleared, folder removed). REST, gateway and QR socket fingerprints unchanged.
- Test `web_page::agent::the_login_page_sees_webview2s_own_agent_and_matching_client_hints` (desktop binary, needs the WebView2 Runtime like the app): opens the real `PageHost` on a hidden window against its own loopback server (never discord.com) and checks `navigator.userAgent` equals the `User-Agent` header seen, is Edge's (`Edg/`) and not kyoko's Chrome string, the brands include `Microsoft Edge WebView2`, the `Chromium` brand version matches the UA major, and `Sec-CH-UA` equals the page's brands. With the Chrome override put back temporarily it failed (reverted). A drop guard removes its profile folder even if it panics. The 43 seam test only lost its Chrome-string assertions.
- 43's first box rewritten to match.
- Checks re-run by the orchestrator in lane C: all green. Mockup parity (port 8493): login-web 4.0%, login-tos 1.6%.
- For later tickets: keep the login window plain WebView2; change only the session's REST and gateway fingerprint if needed. No real sign-in was made (user decision: the first real web login is the check).
