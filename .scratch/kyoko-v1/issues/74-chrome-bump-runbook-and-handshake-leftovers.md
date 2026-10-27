# 74 — Chrome bump runbook and handshake leftovers

**What to build:** A repeatable procedure for moving kyoko to a new Chrome major (headers and handshake together), run once for 157 (full stable 2026-11-03, see v1 59's Answer), and recorded decisions on the handshake details wreq can't or doesn't match. Plan ticket T6; needs the user's decisions where marked.

**Blocked by:** 71, 72, 73

**Status:** claimed

- [ ] The Chrome bump procedure (update local Chrome, `cargo xtask tls-capture --record <major>`, fixture, `CHROME_MAJOR`, profile map, pin bump, TLS seam tests) documented in `docs/fingerprint.md` and run once for 157
- [ ] Decision recorded (user) on the REST header order (`orig_headers`) and the Chrome websocket handshake headers (`Pragma`, `Cache-Control`, `Sec-WebSocket-Extensions` that wreq can't honour), with evidence from public docs or a user-provided capture of the real web client
