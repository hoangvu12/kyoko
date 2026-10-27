# 71 — Gateway over wreq websocket

**What to build:** The gateway socket uses the same Chrome client as REST; zlib-stream handling is unchanged. Plan ticket T3. Lands together with 70.

**Blocked by:** 70

**Status:** resolved

- [x] `gateway` connects through `net::ws`; tokio-tungstenite is no longer a `gateway` dependency
- [x] The User-Agent, Origin and Accept-Language handshake headers are as today (the `wire.rs`/`fingerprint.rs` connection assertions stay green); frame caps and oversize handling unchanged
- [x] Resume, reconnect, 4004/4009 and heartbeat session tests green; zlib-stream frames split across messages still inflate
- [x] A session test captures the gateway (wss) ClientHello and asserts it matches the Chrome wss fixture from 69
- [x] `docs/gateway.md` updated

## Answer

Merged to main as `d6245b6` (worker `53d90ff9`, lane A). Docs: `docs/gateway.md` ("Transport"), `docs/testing.md`.

- `gateway` connects through `net::ws::connect` (headers, frame/message caps, timeout, `ConnectError::Refused(status)` on a refused upgrade, `is_oversized`, `close`); one wreq client per gateway, kept across reconnects; separate from the REST client (different host, Chrome wouldn't share either). tokio-tungstenite is no longer a `gateway` dependency; `tungstenite 0.29` (already in the lock via wreq) is in `[workspace.dependencies]` so `net::ws` can recognise wreq's message-size error. User-Agent, Origin, Accept-Language, 64 MiB caps and kyoko's zlib-stream inflater unchanged.
- New mockcord controls: `split_gateway_messages(parts)` (keeps the `00 00 ff ff` suffix whole in the last message) and `send_uncompressed_gateway_text(text)`. New tests: `zlib_stream_messages_split_across_websocket_messages_still_inflate`, `a_websocket_message_over_the_cap_stops_the_session_as_too_big` (fails if `is_oversized` is broken), `the_gateway_presents_chromes_websocket_client_hello` (0 differences vs `chrome-156-wss.json`), and an upgrade-over-TLS check (ALPN http/1.1, UA, Origin, Accept-Language). All 18 gateway tests green.
- For 72: use `net::ws::connect`; the refused-upgrade status is already there; reuse the wss fixture test. `install_crypto_provider` stays until 72 and 73 drop rustls. For 73: no `chunk()` (use `bytes_stream()`), wreq takes `&str`/`Uri` not `url::Url`, `Trust::Only` or a `CertStore` for theme-link test roots, image destination for media. For 74: the upgrade is lowercase in wreq's order, no `Pragma`/`Cache-Control`/permessage-deflate (`OrigHeaderMap` not used, it can't add deflate), and wreq's decoder adds its own `accept-encoding: zstd,gzip,deflate,br` to the upgrade.
