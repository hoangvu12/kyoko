# 59 — Fingerprint follow-ups from the spike

**What to build:** The corrections ticket 04 found when it compared kyoko's IDENTIFY and REST traffic with the current Discord web client. kyoko keeps presenting one consistent web-client identity.

**Blocked by:** 13 (Versioned read state, persisted client state and late DMs)

**Status:** resolved

- [x] The compiled-in Chrome major matches current stable (156 at the time of the spike) and the `sec-ch-ua` brand string is recomputed for it, in the one fingerprint module
- [x] IDENTIFY follows the web client's key order and drops `large_threshold`; presence is sent as `"unknown"` with `since: 0`
- [x] `client_state` carries `api_code_version`, echoing READY's value when known
- [x] `installation_id` is also sent in IDENTIFY where the web client sends it
- [x] `client_heartbeat_session_id` rotates on the web client's schedule (every 30 minutes per the docs)
- [x] `launch_signature` is added only if its format is documented in public protocol docs; if the only source is a GPL client, it stays omitted and the reason is recorded in `docs/fingerprint.md`
- [x] Session tests assert each change on the wire

Source: `docs/spike.md` (real-traffic comparison table, sources discord.food / userdoccers read 2026-10-25). The fallback build number was already updated to 649389 in ticket 04.

## Answer

Merged to main as `5cef80e` (rebased by the orchestrator; union conflicts with 18's image header constants and mockcord re-exports). Sources recorded in `docs/fingerprint.md` (docs.discord.food / userdoccers master `01624d9`, Chromium Dash, Chromium `user_agent_utils.cc`).

- `CHROME_MAJOR` 156 (full stable 2026-10-20). Bump to 157 after its full stable on 2026-11-03. `sec-ch-ua` is computed from the major by Chromium's GREASE algorithm (`brand_list`): for 156 `"Not:A-Brand";v="8", "Chromium";v="156", "Google Chrome";v="156"`.
- IDENTIFY keys in order: `token`, `properties`, `compress` (false), `presence` `{"activities":[],"status":"unknown","since":0,"afk":false}`, `capabilities`, `client_state` (fresh `{"guild_versions":{},"api_code_version":0}`; echoes READY's `api_code_version` once seen, persisted in the READY snapshot). `large_threshold` removed. `GatewayConfig.identify` became `properties: PropertiesSource`, read on every IDENTIFY (re-identifies pick up the current build number).
- `properties` order ends with `launch_signature`, `client_app_state`, `client_heartbeat_session_id`, then `installation_id` once known. The docs put `installation_id` in IDENTIFY `properties` but not in `X-Super-Properties`, so the two are identical except for that key.
- `launch_signature`: implemented from the public docs (random UUIDv4 with the documented 12 client-mod bits cleared). No GPL code read.
- `client_heartbeat_session_id` rotates after 30 minutes, checked whenever properties are handed out, so REST and the gateway always agree. Clock injectable (`fingerprint::Clock`, `session::client_identity_with_clock`, harness `Options::clock`).
- Mockcord records the raw IDENTIFY text (`GatewayFrame::raw`, `keys(path)`) for key-order tests; its READY carries `api_code_version`. 3 new session tests; existing ones tightened.
- Not sent: op 41 (Update Time Spent Session ID), recommended but optional. `client_app_state: "focused"` kept though absent from the docs' example.
- Confirm on a later real run: the real Chrome 156 `sec-ch-ua`; where the web client puts `installation_id`; `client_app_state`; real `api_code_version` values; presence `"unknown"` resolving to the user's status; Discord accepting `launch_signature`.
