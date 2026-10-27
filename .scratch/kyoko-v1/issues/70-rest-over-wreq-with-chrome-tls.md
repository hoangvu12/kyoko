# 70 — REST over wreq with Chrome's TLS and HTTP/2

**What to build:** All discord.com REST traffic (API, uploads, build-number scrape, email/password login) presents Chrome's TLS and HTTP/2 handshake, with kyoko's headers unchanged. Plan ticket T2 (`.research\tls-plan\plan.md`, sections 3 and 8). Lands together with 71.

**Blocked by:** 69

**Status:** claimed

- [ ] `crates/net` provides the Chrome client builder from `CHROME_MAJOR`'s mapped profile with emulation headers cleared; `rest` uses it for the API, upload and discovery clients
- [ ] `CookieJar` implements wreq's cookie store; cookies still persist, restore and adopt from web login (existing tests green)
- [ ] All existing session and bridge tests pass unchanged against plain-HTTP mockcord
- [ ] A session test asserts no navigation-only header (`upgrade-insecure-requests`, `sec-fetch-user`, `sec-fetch-mode: navigate`, `accept: text/html…`) reaches mockcord on any REST request
- [ ] A session test captures the REST ClientHello and asserts it matches the Chrome fixture from 69 (ignoring GREASE, extension order, random, key shares)
- [ ] A session test completes TLS with ALPN h2 against a local server and asserts Chrome's SETTINGS, WINDOW_UPDATE, HEADERS priority and m,a,s,p
- [ ] `docs/fingerprint.md`, `docs/rest.md`, `docs/testing.md`, `docs/architecture.md`, `THIRD_PARTY_NOTICES.md` updated; `AGENTS.md` hard rule reworded as the user approves (handshake added; `net` in the headless list)
- [ ] `cargo xtask headless-run --mock` passes
