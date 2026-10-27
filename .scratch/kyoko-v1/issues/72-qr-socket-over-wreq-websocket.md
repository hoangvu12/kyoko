# 72 — QR remote-auth socket over wreq websocket

**What to build:** The QR login socket presents the same Chrome handshake as the gateway. Plan ticket T4.

**Blocked by:** 71

**Status:** resolved

- [x] `login::qr` connects through `net::ws`; tokio-tungstenite is no longer a `login` dependency
- [x] QR bridge tests green (scan, confirm, cancel, expire, garbled nonce, captcha on the exchange); refused-upgrade errors still name the HTTP status
- [x] A test captures the remote-auth ClientHello and asserts it matches the wss fixture
- [x] `docs/sign-in.md` updated

## Answer

Merged to main as `d86e6c5` (worker `4e52e7fd`, lane A). Docs: `docs/sign-in.md` ("QR login": transport, caps, timeout, refused status, trust, tests), `docs/fingerprint.md`, `docs/testing.md`, `docs/architecture.md`.

- `crates/login/src/qr/flow.rs`: each attempt builds a client with `net::client_builder(Destination::Fetch, &config.trust)` and connects with `net::ws::connect`; 64 KiB frame and message caps and the 30 s timeout kept; sends `User-Agent`, `Origin: https://discord.com` and now `Accept-Language` (as the gateway and Chrome's wss upgrade do). A refused upgrade is `LoginError::Network("gateway refused the connection with <status>")`; oversize via `ws::is_oversized`; close codes as before. `ws://` only on loopback (mockcord unchanged).
- New `QrConfig.trust: net::Trust` (`Bundled` by default), also used by the ticket exchange's `RestClient`; bridge `qr_config` and xtask's headless run pass `Trust::default()`. `login` depends on `net` instead of tokio-tungstenite; bridge's mockcord dev-dependency turns on `tls`.
- Tests (`bridge::qr_login`, 8 green): `the_remote_auth_socket_presents_chromes_websocket_client_hello` (0 differences vs `chrome-156-wss.json`), `a_refused_remote_auth_upgrade_over_tls_names_the_http_status` (404 named; ALPN http/1.1, `GET /?v=2`, UA, Origin, Accept-Language, upgrade, no authorization).
- kyoko's normal graph no longer has tokio-tungstenite 0.30 or its rustls TLS; the workspace entry (with `rustls-tls-webpki-roots`) is used only by xtask parity's plain `ws://` CDP client (73 drops the TLS feature).
- Checks re-run by the orchestrator in lane A: all green; `tls-capture --compare chrome-156` 0 and 0.
- For 74: the QR upgrade has the gateway's gaps (lowercase names in wreq's order, no `Pragma`/`Cache-Control`/`Sec-WebSocket-Extensions`, wreq's own `accept-encoding`, no `Cookie` line where Chrome sends one on wss).
