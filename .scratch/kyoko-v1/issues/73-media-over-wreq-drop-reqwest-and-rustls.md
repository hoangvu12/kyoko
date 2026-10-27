# 73 — Media and theme links over wreq; reqwest and rustls leave the shipped exe

**What to build:** CDN and GIF media use the Chrome client; theme downloads use wreq too; the release exe no longer links reqwest, rustls or aws-lc. Plan ticket T5.

**Blocked by:** 70

**Status:** resolved

- [x] `MediaFetcher` on wreq: host allow-list on redirects (max 5), https-only outside loopback, size caps, shared in-flight dedupe; media bridge tests green
- [x] A test captures a media ClientHello against an https localhost host and asserts it matches the fixture
- [x] `theme_links` on wreq with `CertStore` for trusted roots; theme-link tests green
- [x] `cargo tree -p kyoko -e normal` shows no reqwest, rustls or aws-lc-sys; `install_crypto_provider` removed
- [x] Release exe size recorded before and after
- [x] `docs/media.md`, `docs/themes.md`, `THIRD_PARTY_NOTICES.md` updated

## Answer

Merged to main as `7c757af` (worker `f02b9129`, lane B). Docs: `docs/media.md`, `docs/themes.md`, `docs/fingerprint.md` (which client each part uses, exe sizes), `docs/testing.md`, `THIRD_PARTY_NOTICES.md` (wreq/BoringSSL section now covers media and theme links; it never listed rustls, aws-lc or reqwest).

- Media: `MediaFetcher` uses `net::client_builder(Destination::Image, &trust)`; new `net::Destination::Image` (HEADERS weight 220). Kept: host allow-list re-checked on every redirect (wreq `Policy::custom`, 5 redirects followed, a 6th refused; reqwest's old check actually followed only 4), https-only outside loopback, 64 MiB cap with or without a declared length, shared in-flight dedupe, image/audio headers. Bodies via `bytes_stream()`. Test-only: `MediaFetcherBuilder::trust(Trust)` and allow-list entries with a port (`localhost:8443`; production entries have none, so 443 only). Bridge `media_fetcher_builder(&fingerprint)` (`media_fetcher` builds on it).
- Theme links: `net::client_builder(Destination::Fetch, ..)`; `ThemeLinkTuning.trusted_roots` becomes `Trust::Only` (a wreq `CertStore`; empty = bundled roots); limits unchanged. The tokio-rustls test server stays a dev-dependency.
- Removed `install_crypto_provider` and reqwest/rustls from `session` and `media`; the workspace `tokio-tungstenite` entry is plain `"0.30"` (xtask parity CDP only). `cargo tree -p kyoko -e normal -i` reqwest / rustls / aws-lc-sys: no match. reqwest remains only as a bridge dev-dependency (`web_login.rs`); rustls/tokio-rustls only behind mockcord's `tls` feature and the theme-link test server.
- New tests `crates/bridge/tests/bridge/media_fetch.rs`: redirects (5 ok, 6th refused and never sent), disallowed redirect host, refusals before connecting (http, non-443 port, userinfo, unknown host, loopback without the mock port), size cap; `media_fetches_present_chromes_client_hello_and_image_request` (0 differences vs `chrome-156-https.json`, ALPN h2, Akamai string, weight 220, pseudo-header order, `sec-fetch-dest: image`, no Authorization or cookie).
- Release `kyoko.exe`: 83,962,880 bytes on `c93d2be` -> 78,870,528 now (-4.86 MiB, includes 72), below the 78,949,888 pre-wreq baseline.
- Checks re-run by the orchestrator in lane B (with the shared BoringSSL prebuilt): all green; `tls-capture --compare chrome-156` 0 and 0.
- For 74: image `Accept` lacks `image/jxl`; no chained HEADERS priorities (every request depends on stream 0); no captured Chrome `<img>` request yet (the fixture page only does `fetch()`): add an `<img>` to the capture page when re-recording for 157.
