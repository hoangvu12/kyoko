# 70 — REST over wreq with Chrome's TLS and HTTP/2

**What to build:** All discord.com REST traffic (API, uploads, build-number scrape, email/password login) presents Chrome's TLS and HTTP/2 handshake, with kyoko's headers unchanged. Plan ticket T2 (`.research\tls-plan\plan.md`, sections 3 and 8). Lands together with 71.

**Blocked by:** 69

**Status:** resolved

- [x] `crates/net` provides the Chrome client builder from `CHROME_MAJOR`'s mapped profile with emulation headers cleared; `rest` uses it for the API, upload and discovery clients
- [x] `CookieJar` implements wreq's cookie store; cookies still persist, restore and adopt from web login (existing tests green)
- [x] All existing session and bridge tests pass unchanged against plain-HTTP mockcord
- [x] A session test asserts no navigation-only header (`upgrade-insecure-requests`, `sec-fetch-user`, `sec-fetch-mode: navigate`, `accept: text/html…`) reaches mockcord on any REST request
- [x] A session test captures the REST ClientHello and asserts it matches the Chrome fixture from 69 (ignoring GREASE, extension order, random, key shares)
- [x] A session test completes TLS with ALPN h2 against a local server and asserts Chrome's SETTINGS, WINDOW_UPDATE, HEADERS priority and m,a,s,p
- [x] `docs/fingerprint.md`, `docs/rest.md`, `docs/testing.md`, `docs/architecture.md`, `THIRD_PARTY_NOTICES.md` updated; `AGENTS.md` hard rule reworded as the user approves (handshake added; `net` in the headless list)
- [x] `cargo xtask headless-run --mock` passes

## Answer

Merged to main as `ee5f161` (worker `53d90ff9`, lane A). Docs: `docs/fingerprint.md` (TLS and HTTP/2 section, "Bumping Chrome", REST header order on H2), `docs/rest.md`, `docs/testing.md`, `docs/architecture.md`, `THIRD_PARTY_NOTICES.md` (BoringSSL, wreq, wreq-util, btls, http2, chromium-roots), `AGENTS.md` (the user-approved wording: "Present the identify properties, REST and gateway headers, and the TLS and HTTP/2 handshake of the Discord web client in Chrome, nothing more; ..."; `net` in the headless list).

- `crates/net` (headless): `profile(major)` maps 154 and 156 to `Profile::Chrome154`; `CHROME_PROFILE` is computed at compile time, so a `CHROME_MAJOR` with no mapping doesn't build. `chrome_emulation(Destination)` (profile headers cleared), `websocket_emulation()` (no ALPS), `client_builder(destination, &Trust)` (no proxy). Trust: wreq's bundled roots by default; tests pass a localhost-only root through `SessionConfig.trust` / `RestConfig.trust` / `GatewayConfig.trust`. Certificate checks are never off.
- HEADERS weight: API and upload requests 220 (Chrome `fetch()`), the build-number scrape 256 (page load; its script fetches also go at 256, Chrome would use less; once per launch).
- `CookieJar` implements wreq's `CookieStore`: one `cookie` field per cookie on H2, one line on HTTP/1.1; persistence and `adopt()` unchanged. `cookie_store` only persists cookies with an expiry (tests seed `Max-Age`).
- Seam tests `crates/session/tests/session/tls_fingerprint.rs`: no navigation header on API, upload, message post or gateway upgrade; REST ClientHello vs `chrome-156-https.json` 0 differences; H2 (ALPN h2, SETTINGS, WINDOW_UPDATE, no PRIORITY frames, weight 220 exclusive on stream 0, m,a,s,p, 2 seeded cookies as 2 fields). Harness options `rest_base`, `gateway_url`, `trust`; `mockcord::hello::chrome_fixture`.
- Release `kyoko.exe` 83,962,880 bytes (+4.78 MiB over 78,949,888) with reqwest, rustls and aws-lc still linked for QR and media. `release.yml` now builds BoringSSL (`-p kyoko` links `net`).
- Checks re-run by the orchestrator in lane A: all green; `cargo xtask tls-capture --client wreq --h2 --compare chrome-156`: 0 and 0 differences.
- For 74: H2 header order is kyoko's insertion order (written in `docs/fingerprint.md`), wreq can't chain priorities like Chrome; image `Accept` lacks `image/jxl`.
