# 72 — QR remote-auth socket over wreq websocket

**What to build:** The QR login socket presents the same Chrome handshake as the gateway. Plan ticket T4.

**Blocked by:** 71

**Status:** claimed

- [ ] `login::qr` connects through `net::ws`; tokio-tungstenite is no longer a `login` dependency
- [ ] QR bridge tests green (scan, confirm, cancel, expire, garbled nonce, captcha on the exchange); refused-upgrade errors still name the HTTP status
- [ ] A test captures the remote-auth ClientHello and asserts it matches the wss fixture
- [ ] `docs/sign-in.md` updated
