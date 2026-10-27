# 73 — Media and theme links over wreq; reqwest and rustls leave the shipped exe

**What to build:** CDN and GIF media use the Chrome client; theme downloads use wreq too; the release exe no longer links reqwest, rustls or aws-lc. Plan ticket T5.

**Blocked by:** 70

**Status:** open

- [ ] `MediaFetcher` on wreq: host allow-list on redirects (max 5), https-only outside loopback, size caps, shared in-flight dedupe; media bridge tests green
- [ ] A test captures a media ClientHello against an https localhost host and asserts it matches the fixture
- [ ] `theme_links` on wreq with `CertStore` for trusted roots; theme-link tests green
- [ ] `cargo tree -p kyoko -e normal` shows no reqwest, rustls or aws-lc-sys; `install_crypto_provider` removed
- [ ] Release exe size recorded before and after
- [ ] `docs/media.md`, `docs/themes.md`, `THIRD_PARTY_NOTICES.md` updated
