# 74 — Chrome bump runbook and handshake leftovers

**What to build:** A repeatable procedure for moving kyoko to a new Chrome major (headers and handshake together), run once for 157 (full stable 2026-11-03, see v1 59's Answer), and recorded decisions on the handshake details wreq can't or doesn't match. Plan ticket T6; needs the user's decisions where marked.

**Blocked by:** 71, 72, 73

**Status:** claimed

- [ ] The Chrome bump procedure (update local Chrome, `cargo xtask tls-capture --record <major>`, fixture, `CHROME_MAJOR`, profile map, pin bump, TLS seam tests) documented in `docs/fingerprint.md` and run once for 157
- [ ] Decision recorded (user) on the REST header order (`orig_headers`) and the Chrome websocket handshake headers (`Pragma`, `Cache-Control`, `Sec-WebSocket-Extensions` that wreq can't honour), with evidence from public docs or a user-provided capture of the real web client

## Progress (part 1, not resolved)

Merged to main as `6a8eeed`, `6c2c023`, `ceaf4a1` (worker `2b57b857`, lane B; checks re-run by the orchestrator: all green, `tls-capture --compare chrome-156` 0 and 0).

- `cargo xtask tls-capture`'s page now also makes a Discord-style GET, message POST, join POST (`X-Context-Properties`), PATCH (`X-Audit-Log-Reason`), a cross-site `<img>` and a cross-site websocket shaped like the gateway, all dummy values; other sites `kyoko-cdn.test`/`kyoko-gateway.test` mapped to 127.0.0.1 by `--host-resolver-rules`. `--compare` skips kinds with no fixture. Fixtures re-recorded (same meaning) plus `chrome-156-image.json`, `chrome-156-wss-cross-site.json`.
- `fingerprint::IMAGE_ACCEPT` now leads with `image/jxl` (Chrome 156); the media seam test checks `accept` and `sec-fetch-*` against the recorded `<img>`.
- `docs/fingerprint.md`: the three order comparisons (API POST, image, websocket upgrade), what wreq can do (`OrigHeaderMap` fixes H2 order fully and upgrade names/case/order; `Connection: Upgrade` value stays lowercase without a wreq patch; no permessage-deflate, never offer it), and the 10-step "Bumping Chrome" runbook.
- Findings: Chrome's order for page-set headers is fixed but neither alphabetical nor insertion order (hash-like; re-capture per bump and whenever kyoko adds a header); Chrome puts `content-length` first on a POST; Chrome chains H2 priority only onto a still-open request, else stream 0 like kyoko.
- Open, not user decisions (plain match-Chrome fixes for part 2): image `sec-fetch-storage-access: active` missing; image `priority: i` vs Chrome's `u=1, i`. Unknown: whether the real web client sends `X-Debug-Options` (kyoko doesn't).
- User decisions asked 2026-10-27 (evidence: `docs/fingerprint.md`, `nguyenvu/.research/tls-plan/t6-evidence.md`): (1) send Chrome's recorded REST header order; (2) websocket upgrade: Chrome's names, case, order, `Pragma`, `Cache-Control`, `Accept-Encoding`, no deflate offer, lowercase `upgrade` left. Orchestrator recommends yes to both. No answer yet.
- Part 2 (after Chrome 157's full stable 2026-11-03, Chrome 157 installed locally): follow the runbook; 157's `sec-ch-ua` should be `"Not-A.Brand";v="99", "Google Chrome";v="157", "Chromium";v="157"` (check against the capture; rename `CHROME_156_BRANDS` in `crates/session/tests/session/fingerprint.rs`); build any order lists from the 157 fixtures.

