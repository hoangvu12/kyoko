# 74 — Chrome bump runbook and handshake leftovers

**What to build:** A repeatable procedure for moving kyoko to a new Chrome major (headers and handshake together), run once for 157 (full stable 2026-11-03, see v1 59's Answer), and recorded decisions on the handshake details wreq can't or doesn't match. Plan ticket T6; needs the user's decisions where marked.

**Blocked by:** 71, 72, 73

**Status:** claimed

- [ ] The Chrome bump procedure (update local Chrome, `cargo xtask tls-capture --record <major>`, fixture, `CHROME_MAJOR`, profile map, pin bump, TLS seam tests) documented in `docs/fingerprint.md` and run once for 157
- [x] Decision recorded (user) on the REST header order (`orig_headers`) and the Chrome websocket handshake headers (`Pragma`, `Cache-Control`, `Sec-WebSocket-Extensions` that wreq can't honour), with evidence from public docs or a user-provided capture of the real web client

## Progress (part 1, not resolved)

Merged to main as `6a8eeed`, `6c2c023`, `ceaf4a1` (worker `2b57b857`, lane B; checks re-run by the orchestrator: all green, `tls-capture --compare chrome-156` 0 and 0).

- `cargo xtask tls-capture`'s page now also makes a Discord-style GET, message POST, join POST (`X-Context-Properties`), PATCH (`X-Audit-Log-Reason`), a cross-site `<img>` and a cross-site websocket shaped like the gateway, all dummy values; other sites `kyoko-cdn.test`/`kyoko-gateway.test` mapped to 127.0.0.1 by `--host-resolver-rules`. `--compare` skips kinds with no fixture. Fixtures re-recorded (same meaning) plus `chrome-156-image.json`, `chrome-156-wss-cross-site.json`.
- `fingerprint::IMAGE_ACCEPT` now leads with `image/jxl` (Chrome 156); the media seam test checks `accept` and `sec-fetch-*` against the recorded `<img>`.
- `docs/fingerprint.md`: the three order comparisons (API POST, image, websocket upgrade), what wreq can do (`OrigHeaderMap` fixes H2 order fully and upgrade names/case/order; `Connection: Upgrade` value stays lowercase without a wreq patch; no permessage-deflate, never offer it), and the 10-step "Bumping Chrome" runbook.
- Findings: Chrome's order for page-set headers is fixed but neither alphabetical nor insertion order (hash-like; re-capture per bump and whenever kyoko adds a header); Chrome puts `content-length` first on a POST; Chrome chains H2 priority only onto a still-open request, else stream 0 like kyoko.
- Open, not user decisions (plain match-Chrome fixes for part 2): image `sec-fetch-storage-access: active` missing; image `priority: i` vs Chrome's `u=1, i`. Unknown: whether the real web client sends `X-Debug-Options` (kyoko doesn't).
- User decisions asked 2026-10-27 (evidence: `docs/fingerprint.md`, `nguyenvu/.research/tls-plan/t6-evidence.md`): (1) send Chrome's recorded REST header order; (2) websocket upgrade: Chrome's names, case, order, `Pragma`, `Cache-Control`, `Accept-Encoding`, no deflate offer, lowercase `upgrade` left. Orchestrator recommended yes to both. **User answered yes to both (2026-10-27).** Part 2a (now, on Chrome 156): implement both plus the two image fixes. Part 2b (after 2026-11-03): the 157 bump, rebuilding the order lists from the 157 fixtures.
- Part 2 (after Chrome 157's full stable 2026-11-03, Chrome 157 installed locally): follow the runbook; 157's `sec-ch-ua` should be `"Not-A.Brand";v="99", "Google Chrome";v="157", "Chromium";v="157"` (check against the capture; rename `CHROME_156_BRANDS` in `crates/session/tests/session/fingerprint.rs`); build any order lists from the 157 fixtures.

## Progress (part 2a, merged)

Merged to main as `dd3b3d0` (worker `79b2c0fe`, lane B; checks re-run by the orchestrator: all green, `tls-capture --compare chrome-156` 0 and 0). The decision box is ticked: the user said yes to both on 2026-10-27.

- `crates/net/src/order.rs`: `API_ORDER` (one ranking that merges the four recorded `/api/` requests, which never disagree), `IMAGE_ORDER`, `UPGRADE_ORDER`, one name per line. `chrome_emulation` applies them per destination (the Document scrape gets none). `content-length` comes first and the cookies come before `priority`. A header that isn't on a list goes after the listed ones in kyoko's order (today only `range` on audio, and `host` on HTTP/1.1). No headers were added or dropped on REST.
- `net::ws::connect` (gateway and QR): Chrome's title case and order with `Host` first, plus `Pragma: no-cache`, `Cache-Control: no-cache` and `Accept-Encoding: gzip, deflate, br, zstd`. Any `Sec-WebSocket-Extensions` is stripped, so deflate is never offered. `Connection: upgrade` stays lowercase (no wreq patch).
- Images: add `sec-fetch-storage-access: active`, use Chrome's order, and go at weight 147 (`IMAGE_WEIGHT` in `net/src/chrome.rs`). `priority: i` is kept: the fixture records `i`, and the `u=1, i` in Progress part 1 was a doc error. Audio shares the media client (order and weight are guesses, since no `<audio>` was captured).
- Tests: `mockcord::hello::order_difference`; `api_requests_send_their_headers_in_chromes_order` (GET, message POST, join, PATCH with reason, over H2 against the hello listener; only `x-debug-options` missing); `uploads_send_their_headers_in_chromes_api_order`; the gateway and QR upgrade tests now check order, case and values against `chrome-156-wss-cross-site.json`; the media test checks order, values and weight 147 against `chrome-156-image.json`. Each was shown to fail on broken code.
- Still different from Chrome: no `X-Debug-Options` (unknown whether the web client sends it), lowercase `upgrade`, no deflate offer, no H2 priority chaining or HPACK choices, the audio guesses, the scrape's order and weight, and any header combination that wasn't captured.
- Part 2b (after 2026-11-03): runbook steps 1–10 in `docs/fingerprint.md`. At step 9, rebuild the three order lists from the 157 fixtures and re-check `IMAGE_WEIGHT`, `sec-fetch-storage-access` and the upgrade's cache headers. Rename `CHROME_156_BRANDS` and check 157's `sec-ch-ua`. The order tests read `chrome-<CHROME_MAJOR>-*.json`.

