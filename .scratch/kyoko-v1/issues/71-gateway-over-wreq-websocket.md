# 71 — Gateway over wreq websocket

**What to build:** The gateway socket uses the same Chrome client as REST; zlib-stream handling is unchanged. Plan ticket T3. Lands together with 70.

**Blocked by:** 70

**Status:** claimed

- [ ] `gateway` connects through `net::ws`; tokio-tungstenite is no longer a `gateway` dependency
- [ ] The User-Agent, Origin and Accept-Language handshake headers are as today (the `wire.rs`/`fingerprint.rs` connection assertions stay green); frame caps and oversize handling unchanged
- [ ] Resume, reconnect, 4004/4009 and heartbeat session tests green; zlib-stream frames split across messages still inflate
- [ ] A session test captures the gateway (wss) ClientHello and asserts it matches the Chrome wss fixture from 69
- [ ] `docs/gateway.md` updated
