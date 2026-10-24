# oxidcord core review: fork viability for a zgui (Rust + Luau) Discord client

Date: 2026-10-25.

**Scope.** A review of the core crates of `Master0fFate/oxidcord` @ `497958b` (v0.1.0-beta.1, MIT OR Apache-2.0), measured against `core.md` Part B. It covers:
- separability of the core from the GPUI UI
- dependency compatibility with `zeronsh/zgui` @ `a9dc282`
- a GPUI → Luau migration estimate

**Method.**
- Both clones were read only. Nothing was built, run or passed to cargo.
- A dependency-closure script read only the two `Cargo.lock` files. It ran from `%TEMP%\oxreview-scripts`, outside the clones.
- Two community-doc pages were fetched to check capability semantics.

**Citations.**
- `path:line` is relative to the oxidcord repo root @ `497958b` unless marked `@a9dc282` (zgui).
- **[read]** means seen in code or docs. **[inferred]** means my conclusion.
- **[community]** means docs.discord.food (reverse-engineered, not authoritative).
- Sections 3, 4 and 5 and parts of 1 build on four parallel read-only sub-reviews (state/session, ui, deps/platform, media/markdown/quality). I spot-checked their headline claims against the source: `MESSAGE_CAP`, `is_spam` unused, `evict_to` uncalled, member-list `resize`, the op 37 payload, ack without debounce, mockcord linked into the desktop binary.

---

## Verdict

**Fork selectively. Don't take it whole, and don't depend on it upstream.**

**Recommended path: (c), vendor a subset and own it.**
- **Take:** `protocol`, `markdown`, `media`, `gateway`, `rest`, `state`, `store`, `login` (token + QR) and parts of `platform`.
- **Rewrite:** `session` as a small facade without voice or plugins.
- **Drop:** `voice`, `plugin-host`, `updater`, `ui`.
- **Treat oxidcord's `session` and `ui` as references (d).**

**Why it's worth it:**
- The code is MIT/Apache, so it carries no GPL problem. Concord's does (core.md B §10.2).
- The core is genuinely GUI-free: no gpui anywhere in its dependency closure.
- It is disciplined. There are zero `unwrap()`s and no `todo!` or `panic!`. Lenient decoding is applied throughout (64 `LenientVec` uses, 102 `#[serde(default)]`), and inputs are bounded.
- It covers far more than the MVP list in core.md B §4: threads, forums, polls, pins, search, moderation, guild admin.
- It fills zgui gaps directly:
  - WebP and animated WebP/GIF/APNG decode
  - a Discord markdown parser
  - tray, global hotkeys, OS notifications, image clipboard
- The UI contract maps cleanly onto zgui's inbox/waker pattern: a `Fn(Changes)` callback, `Arc<RwLock<State>>` reads, and `Command`s in.

**Why not as-is or upstream:**
- The parts that matter most for account safety and protocol correctness are thin or wrong.
- `session` is welded to `voice` (8.8k LOC, cpal/opus/openmls/openh264) and to a wasmtime plugin runtime.
- There are zero tests. The only end-to-end harness runs against a mock that serialises oxidcord's own types.
- Nothing in the repo shows it has been run against real Discord.
- The public history is one 121,994-line drop plus 10 commits, all on 2026-10-23/24.

### Biggest defects

1. **It advertises gateway capabilities it doesn't honour.** It sends `capabilities = 32767` (bits 0–14) (`crates/gateway/src/config.rs:15-16`) [read]. Concord sends 253.

   | Bit | What Discord does when it's set [community] | oxidcord |
   |---|---|---|
   | AUTH_TOKEN_REFRESH | READY may carry `auth_token`, which "the client should discard the current auth token and use" | Field not modelled anywhere (`crates/protocol/src/gateway.rs:265-282`; grep finds no `auth_token`) [read] |
   | DEBOUNCE_MESSAGE_REACTIONS | Sends one MESSAGE_REACTION_ADD_MANY instead of several adds | Not parsed; falls into `Unknown` (`gateway.rs:608-677`) [read] |
   | PASSIVE_GUILD_UPDATE(_V2) | Replaces CHANNEL_UNREAD_UPDATE and voice updates for unsubscribed guilds | Neither is handled [read] |

   - Inferred consequences: a stale stored token after a server-side refresh; reaction counts silently missing; wrong unread badges for every guild you are not viewing.
   - The cost of a wrong fingerprint is undocumented [inferred].
2. **Fingerprint drift from the web client it claims to be** [read; consequences inferred]:
   - It uses API v10 for both REST and gateway (`crates/rest/src/config.rs:6`, `crates/gateway/src/transport.rs:42`). Community docs call v9 the client default.
   - The UA carries a full Chrome version, `Chrome/156.0.8078.48` (`crates/protocol/src/gateway.rs:81-94`). Real Chrome sends the reduced `156.0.0.0` [inferred from Chrome UA reduction].
   - `os_version` is the real Windows build (`gateway.rs:206-224`), where concord forces Chrome's `"10"`.
   - Super-properties lack `client_launch_id`, `launch_signature`, `has_client_mods`, `client_heartbeat_session_id` and `client_app_state` (`gateway.rs:96-128`).
   - There is no cookie jar (the workspace `reqwest` features have no `cookies`, `Cargo.toml` `[workspace.dependencies]`).
   - The Referer is a fixed `channels/@me` (`crates/rest/src/client.rs:31,77-79`).
   - There is no `X-Discord-Timezone`, no installation id, and IDENTIFY sends `large_threshold`.
3. **No account-safety layer.**
   - No DM or new-conversation locks: `is_spam`/`is_message_request` are parsed but never used (grep) [read].
   - No mutation pacing, no 401 stop and no 403 breaker (`crates/rest/src/client.rs:263-302`) [read].
   - Captcha responses appear only as a generic failure. `ApiError` has no captcha fields (`crates/protocol/src/rest.rs:94-100`) [read].
   - Acks are not debounced: one POST per message while you're viewing the tail (`crates/session/src/task.rs:1047-1055`) [read].
   - The bulk-delete UI calls an endpoint that "is not usable by user accounts" [community] (`crates/rest/src/api/channels.rs:395-399`; `crates/session/src/task/commands.rs:906-913`) [read].
4. **The gateway send window is too permissive.**
   - The token bucket starts full at 120 and refills at 2/s, so up to about 240 frames can go out in the first 60 s.
   - Heartbeat, identify and resume bypass the limiter (`crates/gateway/src/session.rs:23-24,78-89,484-489`) [read; arithmetic inferred].
5. **State is unbounded and unversioned.**
   - It keeps 500 messages for every channel ever opened, with no warm/cold eviction.
   - Read-state `version`/`partial` are ignored, and a partial READY wipes read states.
   - The member list only ever subscribes `[0,99]`, is not re-sent on channel switch, and INVALIDATE never re-subscribes.
   - `lazy_private_channels` is dropped.

   (Sub-review [read], see §1.4.)
6. **Storage.**
   - The token and cache key silently fall back to plaintext files whenever the keyring errors (`crates/store/src/token.rs:32-44`, `crates/store/src/seal.rs:65-81`) [read]. That contradicts its own README (`README.md:165`) and architecture doc (`docs/architecture.md:35`).
   - The `messages` table and the media disk cache are never pruned [read].

---

## Per-crate scorecard

Grades are mine against core.md Part B: A = ready, B = minor fixes, C = significant rework, D = not usable.

| Crate | LOC | Correctness vs Part B | Code quality | Coupling | Verdict |
|---|---|---|---|---|---|
| `protocol` | 5,074 (1,928 is an emoji table) | **B**. Lenient models; guild `properties` normalisation for user READY. Ready lacks `auth_token`, `lazy_private_channels`, apex; fingerprint constants are off | A | `windows` only | **Vendor**, fix fingerprint |
| `gateway` | 1,182 | **B−**. The state machine is right (jitter, zombie, resume/reidentify, fatal codes, backoff, resume-URL check, 64 MiB inflate cap). Wrong: v10, capabilities, client_state, send window, op 37/op 8 payloads | A | tokio, tungstenite | **Vendor**, ~300–500 LOC of fixes |
| `rest` | 1,754 | **C**. Bucket learning and 429 handling are OK. Missing: probe, pacing, 401/403 policy, captcha, cookies, per-channel Referer, timezone. v10 | A− | reqwest | **Vendor endpoints**, rewrite the fingerprint and safety layer |
| `state` | 5,009 | **C+**. Broad event coverage; correct permission order. Unbounded caches, no versions, weak member list, single lock | A− | none | **Vendor**, substantial rework |
| `store` | 1,404 | **C+**. SQLite + XChaCha20-Poly1305 is fine. Auto plaintext fallback; no pruning | B+ | rusqlite, keyring | **Vendor**, fix fallback and pruning |
| `session` | 6,554 | **C**. Huge `Command` surface. No safety guards. voice and plugins hard-wired; `expect` on plugin init | B+ | voice, plugin-host, wasmtime | **Reference only**; write a session-lite |
| `login` | 913 | **B−**. Token and QR are correct. Web login works but is brittle (localStorage scraping) and has a mismatched fingerprint | A− | tao, wry, interprocess | **Vendor token + QR**; web login optional, helper binary |
| `media` | 1,142 | **B−**. Formats and URL builder are good. Requests `.gif` not `webp?animated`; disk cache never evicted; no decode concurrency cap | A− | image, reqwest | **Vendor**, fix cache |
| `markdown` | 2,209 | **A−**. Near-complete Discord dialect. Missing `ansi`/`diff`; quotes are line-level | A | zero deps | **Vendor as-is** |
| `platform` | 638 | **B**. Tray, hotkeys, toasts, image clipboard, idle, autostart. `single_instance` duplicates zgui | A− | tray-icon, global-hotkey, notify-rust | **Vendor**, minus single_instance |
| `voice` | 8,845 | not reviewed for correctness (out of MVP scope, core.md B §11) | n/a | cpal, opus, openmls, openh264 | **Drop** |
| `plugin-host` | 4,603 | n/a. wasm WIT plugins whose slot tree is rendered by GPUI. It is also an automation surface | A− | wasmtime 50 | **Drop** (clashes with zgui's Luau plugins) |
| `updater` | 1,513 | n/a. GitHub releases, hard-coded ed25519 key | A− | reqwest | Optional; rekey if kept |

---

## 1. Correctness against core.md Part B

### 1.1 Gateway (`crates/gateway`, `crates/protocol/src/gateway.rs`)

**URL, version, compression, inbound caps**
- The base URL is `wss://gateway.discord.gg`. The query is rewritten to `v=10&encoding=json&compress=zlib-stream` (`crates/gateway/src/config.rs:14`; `crates/gateway/src/transport.rs:30-46`) [read].
  - **Deviation:** Part B recommends v9 to match the browser (core.md B §1.1).
  - v10 is accepted by Discord [official], but it is a fingerprint mismatch [inferred].
- zlib-stream handling [read]:
  - one `Decompress` per connection (`transport.rs:92-105`; created per `Connection::new`, `session.rs:102-112`)
  - buffers until the `00 00 ff ff` suffix (`transport.rs:28,107-126`)
  - **Correct**
- Caps [read]:
  - WebSocket message and frame: 16 MiB (`transport.rs:22,58-60`)
  - pending compressed bytes: 16 MiB (`:113-115`)
  - decompressed output: 64 MiB (`:23,146-148`)
  - Part B's 64 MiB concern is about decompressed READY size, which oxidcord meets. A >16 MiB *compressed* frame would mean roughly >100 MiB decompressed, which is beyond either cap anyway [inferred].
- Oversize is **fatal**: close 1009, then `GatewayEvent::Fatal`, and the session stops (`session.rs:502-505`) [read]. That is acceptable, but the user needs to see it, and session only logs it (§1.4).

**HELLO, heartbeat, zombie check** [read]
- The first beat is jittered `period × rand[0,1)`, and the interval is clamped to 100 ms–300 s (`session.rs:383-388`).
- If a beat comes due while `awaiting_ack` is set, the client closes with 4000 (not 1000/1001) and keeps the session, so it resumes (`session.rs:415-424,491-500`).
- A server op 1 is answered immediately (`:326`). op 11 clears the flag and records RTT (`:347-359`).
- **Matches Part B §1.3.**
- [inferred] Dispatches are pushed into a bounded channel (1024, `handle.rs:18`) with `send().await` (`session.rs:513-518`). If the consumer stalls, socket reads and heartbeat ticks in the same `select!` stall too. A stall longer than the heartbeat interval would then produce a self-inflicted zombie reconnect.

**RESUME vs re-IDENTIFY, INVALID_SESSION, close codes, backoff** [read]
- READY stores `session_id` and `resume_gateway_url`. The resume URL must pass the same wss/loopback check, and `v/encoding/compress` are re-appended (`session.rs:460-466`; `transport.rs:30-46`).
- RESUME is `{token, session_id, seq}`; `seq` defaults to 0 if unknown (`frames.rs:72-81`; `session.rs:393-408`).
- op 7 → close 4000 → resume (`:327-330`).
- op 9:
  - `d:true` with a session → reconnect and resume (`:331-341`)
  - otherwise clear the session and identify after a 1–5 s random delay on the same socket (`:342-344,544-546`)
  - That matches the official guidance.
- Close codes:
  - 4004 and 4010–4014 are fatal (`crates/gateway/src/error.rs:29`)
  - 4007 and 4009 clear the session, so the next connect re-identifies (`session.rs:304`)
  - everything else resumes when a session exists
  - Concord also treats 4015/4016 as fatal; minor.
- Backoff: `2^(n-1)` s with ±25 % jitter, capped at 60 s, reset on READY/RESUMED (`session.rs:538-542,467,473`). Concord starts at 500 ms with full jitter, capped at 30 s; both are reasonable.
- Shutdown sends 1000 (`:242-244`), which invalidates the session as intended.

**Send window: 120 / 60 s** [read; arithmetic inferred]
- A token bucket with capacity 120, starting full, refilling at 120/60 s (`crates/gateway/src/limiter.rs:12-43`; `session.rs:23-24,140-143`).
- A full bucket plus refill allows about 240 frames in the first 60 s, which breaks the official "120 per 60 s".
- Heartbeat, identify and resume are sent through `transmit` directly (`session.rs:404-406,429-432,484-489`), so they are not counted.
- There is one priority queue, not concord's urgent/normal split.
- Queued commands are discarded on every new connection (`session.rs:215`).
- **Fix:** a sliding window that counts every frame, with priorities.

**IDENTIFY contents** (`crates/gateway/src/frames.rs:15-70`) [read]
- Fields: `token`, `capabilities`, `properties`, `presence {status, since:null, activities:[], afk:false}`, `compress:false`, `large_threshold:250`, `client_state:{guild_versions:{}}`. No `intents`.
- **`capabilities = 32767`** (bits 0–14; `config.rs:15-16`). The bits beyond concord's 253, per [community] https://docs.discord.food/gateway/using-gateway:

  | Bit | Name | What it changes | oxidcord handles it? |
  |---|---|---|---|
  | 1 | NO_AFFINE_USER_IDS | Stops member/presence sync and PRESENCE_UPDATE for "implicit relationships" | Harmless, but changes which presences arrive [inferred] |
  | 8 | AUTH_TOKEN_REFRESH | READY may carry `auth_token`; "the client should discard the current auth token and use this in subsequent requests" [community gateway-events] | **No.** `Ready` has no such field (`protocol/src/gateway.rs:265-282`) |
  | 9 | USER_SETTINGS_PROTO | Removes `user_settings`; sends `user_settings_proto` and proto update events instead | **Partly.** Folders are decoded from proto as a fallback (`state/src/ready.rs:45-56`; `state/src/settings.rs:84-95`) |
  | 10 | CLIENT_STATE_V2 | "client state caching v2" (undocumented) | Sends only `guild_versions:{}` |
  | 11 / 14 | PASSIVE_GUILD_UPDATE / _V2 | PASSIVE_UPDATE_V1/V2 replace CHANNEL_UNREAD_UPDATE (and voice state) for unsubscribed guilds; CHANNEL_UNREAD_UPDATE "is not sent" with these | **No.** Neither event is parsed (`gateway.rs:608-677`), so unread state for other guilds goes stale [inferred] |
  | 12 | AUTO_CALL_CONNECT | CALL_CREATE for existing calls without op 13 | CALL_* is parsed as a raw `Value` (`gateway.rs:594-596`) |
  | 13 | DEBOUNCE_MESSAGE_REACTIONS | Sends MESSAGE_REACTION_ADD_MANY instead of several adds | **No.** The event hits `Unknown`, so reactions are lost [inferred] |

  **Recommendation:** send 253, or add each extra bit only once its events are implemented.
- `client_state` [read; inferred]: only `guild_versions:{}`. There is no `highest_last_message_id`, `read_state_version`, `user_guild_settings_version` or `private_channels_version`, so even in-process re-identifies download everything. Part B §2.3 has concord tracking these.
- `presence.status` is `"online"` (`config.rs:25`). Session never overrides it (`crates/session/src/session.rs:86-87`) [read]. Concord sends `"unknown"` on first identify.
- `large_threshold` is part of the community-documented Identify (default 250 for users) [community]. Concord omits it; keep or drop after a real-traffic check [inferred].
- `properties` is the `IdentifyProperties` struct (`protocol/src/gateway.rs:96-156`). It has no `installation_id` and no `has_client_mods`.

**op 37 and GUILD_MEMBER_LIST_UPDATE** [read]
- Payload: `{"subscriptions":{gid:{typing,threads,activities,members:[],channels:{cid:[[a,b]]}}}}` (`crates/gateway/src/handle.rs:64-87`).
  - `member_updates` and `thread_member_lists` are missing.
  - Session always passes `activities:false` and the single range `[0,99]` (`crates/session/src/task.rs:53,1072-1083`).
- It is sent only when the selected **guild** changes, and after READY (`task.rs:780-784,1086-1093`).
  - Switching channels inside a guild never updates `channels`, so the member list stays bound to the first channel opened [inferred from code].
  - There is no scroll range computation and no 100 ms debounce (Part B §3.2).
- List ops (`crates/state/src/member_list.rs:29-72`; `crates/state/src/dispatch.rs:511-617`):
  - SYNC, INSERT, UPDATE and DELETE are applied.
  - INVALIDATE only blanks slots and never re-subscribes. There is no stable/refreshing double buffer (Part B §3.3).
  - `sync`/`insert`/`update` `resize` to any index the server sends (`member_list.rs:36-56`), an allocation-size risk on malformed data [inferred].

**op 8** [read]
- `{guild_id, query, limit}` or `{guild_id, user_ids, limit}` (`handle.rs:89-102`).
- No `nonce`, no `presences`, no single-in-flight scheduler, and no handling of RATE_LIMITED dispatches. Part B §3.4 cites the community limit of "1 request per guild per 30 s".
- Member search is debounced only in the GPUI view (350 ms, `crates/ui/src/shell/member_list.rs:29,127`), so a new UI must re-implement it.

**op 13:** never sent [read]. Fine for receiving DMs (Part B §3.2).

### 1.2 REST (`crates/rest`)

**Base and version** [read]
- `https://discord.com/api/v10` (`crates/rest/src/config.rs:6`).
- QR login uses v9 (`crates/login/src/qr/mod.rs:15`).
- **Deviation from v9** [community: v9 is the client default].

**Header set** (`crates/rest/src/client.rs:46-81,227-250`) [read]

| Status | Headers |
|---|---|
| Present | `User-Agent`, `Accept: */*`, `Accept-Encoding: gzip, deflate, br, zstd`, `Accept-Language`, `X-Discord-Locale`, `X-Super-Properties`, `sec-ch-ua`/`-mobile`/`-platform`, `sec-fetch-*` |
| Conditional | `Origin` only on non-GET to discord.com (`:235-237`); `Authorization` only to the configured host and port over https/loopback (`:213-225`), good |
| Missing | **`X-Discord-Timezone`**, **per-channel `Referer`** (fixed `https://discord.com/channels/@me`, `:31,77-79`), **cookie jar** (no `cookies` feature in workspace `reqwest`), `X-Installation-ID`, `Priority`, `X-Debug-Options` |

- Default headers are fixed when the client is built. `RestClient` has no way to vary the Referer per channel [read].
- Super-properties are base64 of the `IdentifyProperties` struct (`protocol/src/gateway.rs:158-160`), which lacks the launch and session ids Part B §7.1 lists [read].
- `browser_version` and the UA carry the full Chrome version, e.g. `156.0.8078.48`, from chromiumdash stable (`crates/rest/src/api/build.rs:10,25-39`) [read].
  - Real Chrome's UA is reduced to `<major>.0.0.0`, which is what concord uses [inferred].
  - `sec-ch-ua` hard-codes a `Not/A)Brand` GREASE value (`gateway.rs:186-189`) [read].

**Live build-number discovery** [read]
- Mechanism:
  - GET `https://discord.com/app` and take the last six `/assets/*.js` scripts
  - search for `build_number:"` / `buildNumber:"`
  - caps: 2 MiB page, 30 MiB total (`crates/rest/src/api/build.rs:9-15,48-64,97-110`)
  - fallback `647615` (`protocol/src/gateway.rs:88`)
- It runs from `apps/desktop`, not the core, once per 24 h in the background (`apps/desktop/src/main.rs:21,173-175,274-309`).
- The session captures the stored build number *before* the refresh finishes (`main.rs:244-263`), so a new build number only takes effect on the next launch [inferred].
- Token validation and the QR ticket exchange use `RestConfig::default()` with the **fallback** build number and compile-time Chrome version (`crates/login/src/token_login.rs:28-31`; `crates/login/src/qr/flow.rs:167-170`). That is an inconsistent fingerprint across requests [read].

**Rate limits** (`crates/rest/src/limiter.rs`, `client.rs:252-302`) [read]
- Buckets:
  - learned from `X-RateLimit-Bucket` + major parameter (`limiter.rs:31-33,87-121`)
  - route keys template ids, emoji and invite codes (`crates/rest/src/route.rs:33-61`)
  - global 429 blocks everything (`limiter.rs:66-72,123-130`)
- **No probe mode.** An unknown route returns "go" without limit (`limiter.rs:73-75`), so a burst on a fresh route all goes out at once. Concord sends one at a time until `remaining` is known (Part B §5.6).
- 429 handling:
  - waits `retry_after` from the body, then the header, else 1 s; retries up to 3 times if the wait is ≤60 s
  - only for non-POST or explicitly idempotent POSTs; message sends with a nonce count as idempotent (`client.rs:26-29,263-296`; `api/channels.rs:98-104`)
  - Retrying after a 429 is safe, since 429 means nothing was processed [inferred].
- **401/403:** classified per request (`client.rs:152-158`) but with **no global stop on 401 and no 403 breaker** [read].
  - On a gateway 4004, session only logs and shows "disconnected" (`crates/session/src/task.rs:419-422`), and REST keeps going.
  - Every 401/403/429 counts toward the official 10,000 per 10 min invalid-request IP ban [inferred].
- **Captcha:** a captcha 400 becomes `RestError::Http(400, Some(ApiError{code,message,errors}))`. `ApiError` has no `captcha_key`/`captcha_sitekey` fields (`crates/protocol/src/rest.rs:94-100`) [read].
  - It is never auto-retried, which is good. But nothing tells the user "finish this in the official client".
  - A failed send is marked failed with `reason: None` (`task.rs:512-514,1004-1007`), so a user who keeps clicking retry is re-triggering the captcha. Concord says that escalates to a block (Part B §6.4) [inferred].
  - Only the QR ticket exchange maps 400 to a captcha hint (`login/src/qr/flow.rs:249-263`) [read].
- **Mutation pacing:** none in rest or session (`client.rs:263-302`; sub-review grep of session) [read]. Part B §5.6 has 500 ms between mutations and 100 ms between sends.

**Endpoints** (`crates/rest/src/api/*`) [read]
- Messages, reactions, typing, ack, ack-bulk, pins, threads, forums, polls, search, invites, relationships, profile, settings, sessions, guild admin, expressions and GIFs.
- `CreateMessage` has no `enforce_nonce` and no `mobile_network_type` (`protocol/src/rest.rs:38-57`); concord sends both. Without `enforce_nonce`, a send retried after a network error (not a 429) could duplicate [inferred].
- `bulk_delete_messages` (`api/channels.rs:395-399`) [read]: docs.discord.food says "This endpoint is not usable by user accounts" [community]. The multi-select delete UI drives it (`session/src/task/commands.rs:906-913`).
- Ack body is `{"token":null,"last_viewed":<day>}` (`task.rs:1051`), the same as concord [read].

### 1.3 Login (`crates/login`)

**Token** [read]
- Trims quotes and backticks, then checks the token is printable ASCII and ≤512 bytes (`token_login.rs:10-19`).
- Calls `GET /users/@me` and maps 401 to `InvalidToken` (`:22-36`; `error.rs:27-38`). Matches Part B §6.1, apart from the fallback fingerprint noted above.

**QR remote auth** [read] (`crates/login/src/qr/*`)
- Steps:
  - `wss://remote-auth-gateway.discord.gg/?v=2` with `Origin: https://discord.com` (`qr/mod.rs:13-14`; `qr/flow.rs:86-108`)
  - RSA-2048 key with SPKI DER, base64 (`crypto.rs:12,23-36`)
  - OAEP-SHA256 decrypt (`:42-50`)
  - nonce proof in base64url without padding (`:59-61`)
  - fingerprint check `== base64url(sha256(SPKI))` before showing `https://discord.com/ra/{fp}` (`flow.rs:222-229`)
  - user payload split `id:disc:avatar:username` (`:231-247`)
  - ticket → `POST /api/v9/users/@me/remote-auth/login` → decrypt the token (`:249-253`)
- Heartbeat is jittered with an ACK check (`:177-193`). The session deadline comes from `timeout_ms` (`:153,184`).
- **Matches Part B §6.3 step for step.**
- Gaps:
  - no `User-Agent` on the WebSocket and no `X-Fingerprint` on the ticket exchange (Part B §6.3 step 8)
  - 4003 ends as `Expired` with no automatic reconnect (concord retries 3 times)
  - `deny.toml:7` ignores RUSTSEC-2023-0071 (rsa Marvin timing), which is reasonable for a one-shot key

**Web login** (wry/tao) (`crates/login/src/web_login.rs`) [read]
- Flow:
  1. The parent binds a random local socket `oxidcord-login-<128-bit hex>` (`:85-100,183-187`).
  2. It re-executes itself with `--login-helper <name>` and `CREATE_NO_WINDOW` (`:216-227`; `apps/desktop/src/main.rs:121-123,322-333`).
  3. The helper opens a tao window and a wry webview on `https://discord.com/login` with a throwaway data dir in `%TEMP%` (`:243-268`).
- **Token extraction:**
  - An init script, top frame only and `location.origin === 'https://discord.com'`, captures `window.localStorage` at document start.
  - It falls back to a hidden iframe's `localStorage` when the page has deleted it.
  - It polls `getItem('token')` every second and `ipc.postMessage`s it (`:46-80`).
  - The IPC handler re-checks host `discord.com` (`:288-296`). The parent reads ≤1024 bytes with a 5 s timeout and validates printability (`:168-181`).
  - The webview's browsing data is cleared and its data dir removed (`:205-214,284`).
- **Safety:** reasonable [inferred].
  - The token never touches disk outside the keyring path.
  - The IPC name is unguessable and the listener exists before the helper starts.
  - Only same-user processes, which can already read the keyring, could interfere.
  - It does solve the captcha problem properly: the user completes hCaptcha in a real browser engine.
- **Robustness:** brittle [inferred].
  - It depends on Discord's web client still writing the token to `localStorage["token"]`, and on the "deleted localStorage → iframe" workaround (a well-known token-grabber technique).
  - The account's new session is created by **Edge WebView2** (Windows) or **WebKitGTK** (Linux), with their own UA. The client then reconnects claiming to be Chrome. That is a login-device vs session-fingerprint mismatch.
- **For us:**
  - keep token + QR
  - make web login optional, in a **separate helper binary**, so tao and wry don't link into the zgui executable
  - on Windows, consider zgui's own `webview2` feature (§4.2)

### 1.4 State (`crates/state`) (sub-review, spot-checked)

**Model** [read]
- `pub type SharedState = Arc<RwLock<State>>` (parking_lot) with flat maps (`crates/state/src/state.rs:24,40-73`).
- No per-area Arc/COW and no revision counters. Each apply returns a `Changes` dirty-set: 15 bools plus 8 `IdSet`s that collapse to `All` above 32 ids (`crates/state/src/changes.rs:4-90`). READY returns `Changes::everything()` (`crates/state/src/ready.rs:58`).
- `docs/architecture.md:27` claims "`watch`-style revision counter plus `Arc` snapshots", which is doc drift.
- The UI reads under the read lock through row builders that return owned view-models (`crates/state/src/rows.rs:141,166,290,328,424`; `crates/state/src/views.rs:7-127`).
- `channel_rows` rebuilds the whole row vector on each call (`rows.rs:328-373`).
- [inferred] A large READY apply holds the write lock for its whole duration. For zgui that's fine as long as projection runs on the UI thread, after the `Changes` ping.

**Message cache** [read]
- `MESSAGE_CAP = 500` per channel (`crates/state/src/messages.rs:9`).
- No limit on the number of channels and no warm/cold redaction. Entries leave only on channel or guild delete (`dispatch.rs:351,415`).
- Part B §8 has concord at 200 × 10 warm channels.
- Reaction add does `count += 1` with no dedupe (`messages.rs:297-314`).

**Permission math** (`crates/state/src/permissions.rs`) [read]
- Correct:
  - owner → all; @everyone plus roles; ADMINISTRATOR → all (`:26-42`)
  - overwrites in Discord's order (`:65-96`)
  - threads inherit the parent (`:55-63`)
  - timeout masks to VIEW_CHANNEL | READ_MESSAGE_HISTORY (`:18,46-51`)
- The implicit rules (no VIEW_CHANNEL strips everything) are covered only because the helpers OR in VIEW_CHANNEL (`:129-152`).
- Session's `channel_allows` checks only the single bit and returns true for any DM (`crates/session/src/task/actions.rs:62-71`).

**Read state** [read]
- READY entries are filtered to channel type (`ready.rs:34-41`).
- `version` and `partial` are decoded but never used. `reset_for_ready` always clears read states (`ready.rs:110`), so a `partial:true` READY drops entries [inferred].
- MESSAGE_ACK overwrites unconditionally and ignores `version` (`dispatch.rs:293-304`).
- CHANNEL_UNREAD_UPDATE, USER_NON_CHANNEL_ACK and GUILD_FEATURE_ACK are not parsed (`protocol/src/gateway.rs:608-677`).

**READY/READY_SUPPLEMENTAL fields** [read]
- READY consumes: `user`, `guilds` (with `properties` normalisation, `protocol/src/guild.rs:139-169`), `private_channels` (recipients interned, `recipient_ids` filled), `relationships`, `read_state`, `user_guild_settings`, `users` (dedupe), `merged_members`, `user_settings`, `user_settings_proto`, `session_id`, `resume_gateway_url` (`protocol/src/gateway.rs:265-282`; `ready.rs:17-56,176-205`).
- READY ignores: `sessions` (parsed, unused), `apex_experiments`, `auth_token`, `country_code`, `notification_settings`, READY `merged_presences`.
- SUPPLEMENTAL consumes only `merged_presences`, `merged_members` and `guilds[].voice_states` (`ready.rs:61-101`). **`lazy_private_channels` is dropped**, so DMs the server omitted as "already cached" never appear [inferred].

**Persistence**
- Session stores the whole READY JSON and guilds/channels/users/read states in SQLite, then replays them as a synthetic READY on startup (`session/src/persist.rs:105-140`; `task.rs:321-345`) [read].
- That's an offline UI cache only; no `client_state` versions are persisted.

**Dispatch coverage vs Part B §4**
- Handled: everything in the table except RELATIONSHIP_UPDATE, CHANNEL_RECIPIENT_ADD/REMOVE and the ack variants above (`dispatch.rs:33-167`) [read].
- A dispatch that fails typed decode is dropped whole, with a debug log (`gateway/src/session.rs:447-456`; `session/src/task.rs:426-428`) [read].

### 1.5 Store (`crates/store`)

**SQLite schema** (`crates/store/src/migrations.rs:5-52`) [read]
- Tables: `kv`, `guilds`, `channels(guild_id)`, `users`, `messages(channel_id, id DESC)`, `read_state`, `drafts`, `media_cache`.
- `user_version` migrations; WAL, `synchronous=NORMAL` (`database.rs:94-105`).

**Encryption** [read]
- XChaCha20-Poly1305 with a random 24-byte nonce and the `OXC1` magic as AAD (`seal.rs:9-58`).
- Sealed columns: kv.value, guilds/channels/users/messages json, drafts (`database.rs:13-20`).
- Ids and timestamps are clear (README states this, `README.md:165`).
- A canary row detects a key change and **drops** the sealed tables rather than failing (`database.rs:358-389`). Good.

**Token and key handling** [read]
- keyring 4.2 with service `oxidcord` and users `session-token`, `account-<id>`, `cache-key` (`token.rs:9-10,77-99`; `accounts.rs:109-114`; `seal.rs:13`).
- **Any keyring error silently writes the secret to a plaintext file**, regardless of the `plaintext_token` setting, with only a `warn!` (`token.rs:32-44`). The same applies to the cache key (`seal.rs:65-81`, commit a5ab547 "Fall back to the key file when no credential store exists").
  - On Windows the file gets no ACL hardening (`atomic.rs:34-41`) and lives in the `directories` config dir (`paths.rs:18`), which is Roaming AppData [inferred].
  - This contradicts `README.md:165` and `docs/architecture.md:35`.
  - core.md B §6.5 recommends: no plaintext on Windows; use DPAPI or refuse to persist.
- If the keyring read fails transiently, a **new** cache key is generated and saved (`seal.rs:67-80`). The canary then wipes the cache: data loss, not corruption [inferred].
- Logout clears tokens and the READY kv entry, but cached messages, drafts and read states stay (sub-review, `task.rs:748-754`; `persist.rs:349-353`).

**Bounds** [read]
- `messages` rows are never pruned (`grep prune` finds nothing; only per-id deletes, `database.rs:167`).
- The parallel `media_cache` table API has no caller.

### 1.6 Media (`crates/media`) (sub-review, spot-checked)

**Formats** [read]
- PNG/APNG, GIF and WebP (still and animated) decode to RGBA frames. JPEG, BMP and ICO decode as stills. AVIF fails. Lottie stickers are unsupported (`crates/media/src/decode.rs:36-78`; `cdn.rs:114`).
- **This fills zgui's WebP gap** (core.md A §5).

**Limits** [read]
- 32 Mpx per image, 500 frames, 64 Mpx total frame pixels, 64 MiB input (`decode.rs:25-34,115-121`).
- That is far looser than concord's 4096² / 32 frames. A single animation can reach about 256 MiB of RGBA [inferred].

**URLs** [read]
- `cdn.discordapp.com` and `media.discordapp.net`; sizes clamped to powers of two from 16 to 4096 (`cdn.rs:3-4,56-58`).
- Default avatar `(id>>22)%6`, or `discrim%5` for legacy users (`:84-90`). Member avatars (`:176-182`).
- Deviation: `a_` hashes and animated emoji request `.gif`, not `.webp?animated=true` as the official docs advise (`:64-66,100-106`).
- `attachment_proxy` keeps the signed `ex/is/hm` params but never adds `animated=true`, so animated attachments arrive as still WebP (`:142-174`; `ui/src/shell/message/media.rs:82`) [inferred].

**Fetch** [read]
- One pooled, https-only, **unauthenticated**, cookie-less client with a host allowlist, redirect re-checks and a 64 MiB streamed cap; in-flight dedupe (`fetch.rs:14-26,68-85,126-148,176-211`). Matches Part B §5.1/§9.3.
- No connect timeout separate from the 30 s total [read].

**Cache** [read]
- Memory LRU at a 256 MiB byte budget, no entry cap (`cache.rs:18,64-119`).
- **The disk cache is never evicted.** `DiskCache::evict_to` (`cache.rs:173-207`) has no caller (grep) [read].
- No limit on concurrent fetches or decodes (`service.rs:71-101`); concord uses 2 workers.
- `MediaService::new` panics outside a tokio context (`service.rs:50`) [inferred].

### 1.7 Markdown (`crates/markdown`) (sub-review)

**Covered** [read]
- Inline:
  - bold/italic/underline/strike/spoiler (`inline.rs:95-96,141-240`)
  - inline code (`:260-278`)
  - fenced code with ~16-family highlighting (`fence.rs:11-55`; `highlight/languages.rs`)
  - `@everyone`/`@here` (`inline.rs:280-297`)
  - backslash escapes, with depth and blow-up guards (`inline.rs:6,78-89,141-162`)
- Blocks:
  - `>` and `>>>` quotes (`block.rs:75-91`)
  - `#`–`###` headings and `-#` subtext (`:102-137`)
  - lists nested to depth 2 (`:168-232`)
- Links and entities:
  - masked links with `<url>` suppression and autolinks (`link.rs:22-107`)
  - `<@id>`, `<@!id>`, `<@&id>`, `<#id>`, custom and animated emoji, `<t:…:style>`, `</cmd sub:id>`, `<id:customize|browse|guide|linked-roles>` (`entity.rs:4-128`)
- jumbo-emoji detection (`document.rs:21-68`)
- Zero dependencies; the AST is a plain enum (`ast.rs:1-50`).

**Missing** [read]
- `ansi` and `diff` code blocks.
- Consecutive `>` lines are separate quote nodes with inline-only content.
- Unicode emoji are not emitted as nodes.

**For zgui** [inferred]: inline nodes map to `ui.rich` spans. Custom emoji hit zgui's square, static span-icon limit (core.md A §5).

### 1.8 Account-safety guards

| Guard (core.md B §7.2) | oxidcord | Evidence |
|---|---|---|
| DM / new-conversation lock (spam, message request, empty or low-history DM) | **None** | `is_spam`/`is_message_request` unused outside `protocol` (grep); `OpenDm` POSTs immediately (`session/src/task.rs:1094-1105`) [read] |
| History loaded before send | **None** | `send_message` checks only for a signed-in user (`task.rs:947-975`) [read] |
| Typing cadence | UI 5 s; session throttles 9 s per channel | `ui/src/shell/composer.rs:31,286-296`; `task.rs:50,1011-1024` [read]. OK |
| Ack debounce | **None.** One POST per new tail message while focused | `ui/src/shell/timeline.rs:236-255`; `task.rs:1026-1055` [read] |
| Mutation pacing | **None** | §1.2 |
| 401 stop / 403 breaker / captcha → stop | **None** / **None** / not surfaced | §1.2 |
| op 8 scheduling, op 37 debounce | **None** | §1.1 |
| Automation surface | Plugin broker can send messages and reactions as the user (permission-gated, rate-limited); first-party `delete-keeper`/`edit-history` plugins | `session/src/broker.rs:523-540`; `plugin-host/src/host_state.rs:207-213`; `plugins/` [read]. A ToS-policy problem per core.md B §7 [inferred] |

---

## 2. Completeness and quality

**Stubs and panics** [read; sub-review grep over core crates]
- 0 `todo!`, `unimplemented!`, `unreachable!` or `panic!`; 0 `TODO`/`FIXME`; 0 `.unwrap()`.
- 7 `.expect(`, none on network data:
  - `rest/src/client.rs:169`, `rest/src/config.rs:33`
  - `session/src/session.rs:56,113`
  - `login/src/qr/mod.rs:27,29`
  - `media/src/cache.rs:146`
- `session.rs:113` (`"plugin runtime initializes"`) crashes the client if wasmtime fails to initialise.

**Lenient decoding is really applied** [read]
- Counts: 64 `LenientVec`, 102 `#[serde(default)]`, 29 `int_enum!` generating `Unknown` variants; snowflakes accept number or string (`protocol/src/snowflake.rs:57-66`); `LenientVec` drops bad elements via `RawValue` (`protocol/src/lenient.rs:11-120`).
- 25 plain `Vec` fields remain (e.g. member `roles`). One bad id there fails the parent object [inferred].

**Error handling** [read]
- 76 `let _ =`, mostly channel sends and cleanup.
- Swallowed or log-only paths:
  - `spawn_rest` (edit/delete/reaction/typing/ack)
  - history load failures
  - persist queue overflow (`session/src/persist.rs:51-55`)
  - command queue overflow: `try_send` on 256 (`session/src/session.rs:161-165`)
  - gateway Fatal, which only logs plus "disconnected" (`task.rs:419-422`)

**Mock-only paths and seams**
- Plain `ws://` and `http://` only for loopback (`gateway/src/transport.rs:30-55`, `rest/src/client.rs:90-101`, `login/src/qr/flow.rs:49-64`, `media/src/fetch.rs:207-211`) [read]. Safe.
- The release desktop binary links `mockcord` (axum) unconditionally (`apps/desktop/Cargo.toml:19`; `main.rs:159-171`) [read].
- The UI rewrites CDN URLs to the mock origin under `--mock` (`ui/src/images.rs:128-141`) [read].
- **What the mock can't catch** [read: `tools/mockcord/src/gateway.rs`, `ready.rs`, `rest/mod.rs`]:
  - it doesn't validate capabilities, properties, `client_state` or the send window
  - it never sends op 7, op 9 `true` or a zombie
  - it has no captcha
  - it serves REST under `/api/v10` only and emits `"v":10`
  - its READY is built by serialising oxidcord's **own** protocol types, with `merged_members` holding only the self user
- So the mock round-trips the client's own model, and bugs such as `bulk-delete` for users and unknown capability-driven events can't surface [inferred].

**Evidence of real-Discord use**
- No recorded fixtures (only flatpak, catalog and theme JSON) [read].
- The README's performance chart was measured "against its built-in mock server" (`README.md:44`) [read].
- Knowledge of real traffic is visible [read]:
  - the guild `properties` normalisation (`protocol/src/guild.rs:139-169`)
  - the `user_settings_proto` decoder
  - the build-number scrape keys
  - klipy/Tenor/Spotify media hosts
  - `fail_if_not_exists:false` on replies (`task.rs:991`)
- But **nothing proves an end-to-end run against real Discord** [inferred].
- CI runs fmt, clippy `-D warnings`, `build --locked`, `xtask headless-run --mock`, a wasm plugin build and `cargo deny`. No `cargo test` (`.github/workflows/ci.yml:19-44`) [read].

**Process signal** [read; reading inferred]
- `AGENTS.md:7-8` forbids comments and unit tests.
- History: commit `d7cb21d` adds 567 files / 121,994 lines in one go; the other 10 commits land within about 4 hours on 2026-10-23/24 (`git log`).
- Reading: an agent-generated codebase with high surface polish and shallow protocol validation.
- `AGENTS.md:15` claims a clean-room process. We can't verify provenance; no concord-specific identifiers were spotted.

**Overall:** clean, consistent, defensive Rust whose *breadth* is far ahead of its *protocol depth*. Treat it as a well-typed starting point that needs a correctness and safety pass and a real test suite, not as a proven core.

---

## 3. Separability

**Public API a UI consumes** [read]

- **Startup** (`crates/session/src/session.rs:64-155`):
  - `Session::start / start_with / start_with_plugins(SessionConfig, VoiceIo, PluginOptions)`.
  - `SessionConfig` (`crates/session/src/config.rs:18-50`) holds store, rest_base, gateway_url, token, identify, locale, an optional tokio `Handle`, and `notify`.
- **Event subscription:** `notify: NotifyFn = Arc<dyn Fn(Changes) + Send + Sync>`.
  - It is called on a tokio worker through a coalescing `Notifier`: at most 60 events per 16 ms window (`crates/session/src/notifier.rs:8-49`).
  - There is no channel or watch. The UI merges `Changes` (`Changes::merge`) and read-locks `SharedState` (`session.state() -> &SharedState`).
  - The GPUI app pushes `Changes` into an unbounded channel drained by a GPUI task (`ui/src/app.rs:192-223,320-327`).
- **Snapshot / revision types:** none. `state::Changes` (dirty-set) plus view-model builders over `&State` (`state/src/rows.rs`, `state/src/views.rs`: `MessageRow`, `MessageView`, `ChannelRow`, `ChannelTreeNode`, `DmRow`, `GuildSummary`, `MemberRow`, `MemberListRow`).
  - Rendered markdown arrives as `RenderedMessage{ast, hidden, revision}` from a "plugin-render" thread (`session/src/render.rs:16,133-197`; `state/src/state.rs:351-366`).
- **Commands:** `session.send(Command)` (`try_send`; dropped when full). `Command` is about 149 variants (`crates/session/src/command.rs:14-555`), grouped as:
  - messaging (select/load/send/edit/delete/react/ack/draft/search/forward/poll/pins/bulk delete)
  - presence
  - guild navigation (SubscribeGuild, SearchMembers, OpenDm)
  - threads and forums
  - social and relationships
  - notifications and read
  - guild admin and moderation
  - expressions and GIFs
  - voice and video (~25)
  - plugins (14)
  - accounts and lifecycle
- **For zgui** [inferred]: `NotifyFn` pushes into a `Mutex<Changes>` + `Waker` inbox (not `zgui::task::latest`, because `Changes` must merge, not replace; core.md A §3). The UI thread then read-locks, projects into per-area `Shared` values, and maps `ui.invoke` to `Command`.

**GPUI or other unacceptable dependencies in the core** [read]
- No core crate's dependency closure contains gpui, winit, taffy, cosmic-text, mlua or accesskit. Those appear only under `oxidcord-ui` (lock-closure script).
- Unwanted but not GPUI:
  - `session` pulls **voice** (cpal, opusic-sys via cmake, openh264 `source` via cc+nasm, openmls) and **wasmtime 50 + cranelift**, both non-optional (`crates/session/Cargo.toml`; only feature `mock = ["voice/testing"]`).
  - `login` pulls tao/wry, which means gtk/webkit2gtk on Linux.
  - `protocol` pulls `windows` for `RtlGetVersion` (`protocol/src/gateway.rs:206-224`).
- Cutting voice and plugins out of session is a **source edit**, not a feature flag [read file sizes; effort inferred]:
  - voice-specific session code: about 1.2k LOC plus `Command`/`Internal` variants
  - plugin-specific: about 1.4k LOC; render must become plain `markdown::parse`
- `xtask headless-run` drives `Session` with no UI at all (`tools/xtask/src/headless.rs`), which proves headless operation [read].

**plugin-host and the "UI slot renderer"** [read]
- `plugin-host` has no GPUI dependency. It produces a declarative WIT `ui-tree` (column/row/text/button/input/toggle/image/badge/divider/icon/progress/link) stored in `Slots` (`crates/plugin-host/wit/oxidcord.wit:107-180,329,342`; `crates/plugin-host/src/slots.rs:48-119`).
- **Rendering happens in GPUI** in `crates/ui/src/shell/plugins/slot_renderer.rs:155-205` (532 LOC).
- `docs/architecture.md:16` lists the renderer under plugin-host; that is doc drift.
- It is hard-wired into session (runtime init with `expect`, every message passes through `filter_message`/`render_message`, before-send hooks) (`session/src/session.rs:102-118`; `render.rs:154-167`; `task.rs:970-974`).
- **Recommendation [inferred]:** drop it. zgui already has a sandboxed Luau plugin and slot model (core.md A §3), and two plugin systems is one too many.

**Threading** [read]
- Session owns or borrows a multi-thread tokio runtime (`session.rs:47-61`; 4 workers if none is given). It installs the aws-lc-rs rustls provider (`:43-45`).
- Nothing expects a GPUI context. `Session` is `Send`, `!Sync` (it holds a `std::sync::mpsc::Receiver`) [inferred].
- `MediaService::new` needs an entered tokio context (`media/src/service.rs:50`).

---

## 4. zgui dependency compatibility

### 4.1 Patches, versions, MSRV

**zgui's required `[patch.crates-io]`** (`Cargo.toml:69-92 @a9dc282`) [read]:

| Crate | Version |
|---|---|
| wgpu-hal | 29.0.4 |
| cosmic-text | 0.19.0 |
| taffy | 0.9.0 |
| accesskit_macos | 0.26.3 |
| accesskit_atspi_common | 0.19.1 |
| accesskit_unix | 0.22.1 |
| accesskit_windows | 0.34.0 |
| winit | 0.30.13 |
| mlua | 0.12.2 |

- **None of these crates is in any oxidcord core crate's dependency closure**, so the patches don't touch the core [read via lock closure].

**Shared crates** (348 in both locks) [read; outcomes inferred]:
- Identical in both: `windows` 0.62.2, webview2-com 0.39.1, image 0.25.10, rfd 0.17.2, raw-window-handle 0.6.2, tungstenite 0.30, zbus 5.19, flate2/zlib-rs.
- Semver-compatible:
  - arboard 3.7 vs 3.6.1 resolves to one copy
  - `blocking` 1.7 vs zgui's Linux `=1.6.2` pin: the requirements checked allow 1.6.2. oo7, async-net and smol were not checked.
- Real duplicate: **macOS only**, tray-icon's muda 0.21 + keyboard-types 0.8 alongside zgui's muda 0.18 + keyboard-types 0.7 (`crates/zgui-desktop/Cargo.toml:90-91 @a9dc282`). Irrelevant on Windows and Linux.
- **tokio, reqwest, rustls:** zgui has none, so there is only one copy of each.
  - Both ring and aws-lc-rs get built; session selects aws-lc-rs. That adds a CMake build dependency [inferred from lock].
- **MSRV** [read; inferred]:
  - oxidcord pins toolchain 1.99.0 (`rust-toolchain.toml`) with `rust-version = "1.99"` and resolver 3 (`Cargo.toml:2,11`).
  - zgui's `rust-version = "1.95"` is a floor; CI uses stable (`Cargo.toml:15,25 @a9dc282`).
  - A combined workspace on 1.99 with edition 2024 is fine; 1.99 is the current stable at this date.

### 4.2 login: tao/wry vs zgui's winit

- tao 0.37.1 is its own winit fork and doesn't depend on winit, so zgui's winit patch can't reach it.
- tao uses the same `windows` 0.62.2, and wry the same webview2-com 0.39.1, as zgui [read, lock].
- The two event loops never share a process: web login runs in a re-executed helper (§1.3) [read].
- Even so, tao and wry **link into the main binary**. Moving them to a separate `login-helper` binary keeps the zgui executable clean [inferred].
- zgui's `web_view` (WebView2 on Windows) can replace wry there [read @a9dc282]. It has:
  - ephemeral `WebProfile` and document-start `UserScript` (`crates/zgui/src/web_view.rs:43-65,220-259`)
  - message events with origin (`:332-345`)
  - the `postMessage` bridge (`crates/zgui-desktop/src/native_host/webview2.rs:62-63`)
- It is not available on Linux: "`zgui::web_view` has no Linux implementation" (`docs/linux.md:368-411 @a9dc282`).

### 4.3 `platform` pieces vs zgui gaps

zgui gaps are from core.md A §7.

| Module | Fills a zgui gap? | Event-loop needs and integration |
|---|---|---|
| `tray.rs` (tray-icon; ksni on Linux) (`crates/platform/src/tray.rs:9,56-85`) | **Yes** (zgui has no tray) | Windows needs a message pump on the creating thread [read: `docs/architecture.md:43`]. zgui's patched winit drains **all** thread messages with `PeekMessageW(..., 0, 0, 0, PM_REMOVE)` + `DispatchMessageW` (`vendor/winit/src/platform_impl/windows/event_loop.rs:408-419 @a9dc282`) [read]. So create the tray inside the `Application::run` closure on the UI thread (`crates/zgui-desktop/src/application.rs:736 @a9dc282`) [inferred]. Events are an mpsc `Receiver`: forward via a thread to the `Send + Sync` `WindowHandle` (show/minimize/focus/close/wake, `application.rs:265-404,6066-6070 @a9dc282`) or into the UI inbox |
| `hotkeys.rs` (global-hotkey) (`:12,27-53`) | **Yes** | Same as tray. X11 only on Linux [inferred] |
| `notifications.rs` (notify-rust; AUMID) (`:21-46`) | **Yes** (zgui has no OS toasts) | Any thread. The AUMID setter duplicates zgui's `application_id`; pick one |
| `clipboard.rs` image read/write (`:14-37`) | **Yes** (zgui has text and HTML only) | Any thread |
| `idle.rs`, `autostart.rs` | Yes (auto-away, launch at login) | Any thread |
| `single_instance.rs` (`:10,30-99`) | **No**: zgui has a named pipe / D-Bus via `application_id` → `OpenUrls` (`docs/platform-services.md:124-136 @a9dc282`) | Drop it, or the two will double-claim |
| `open.rs`, `system_theme.rs` | No (zgui has `open_url` and system settings) | n/a |

zgui exposes no public raw HWND accessor (only a private `HasWindowHandle` in `file_dialog.rs:387 @a9dc282`). `WindowHandle::set_visible` covers oxidcord's `ShowWindow` use (`crates/ui/src/desktop.rs:130-170`) [read; inferred].

---

## 5. Migration estimate (GPUI `crates/ui` → zgui Luau)

**Inventory** [read via `wc` and grep; class split partly inferred]
- `crates/ui` is 41,313 LOC; 110 of 133 files import gpui.

| Class | Description | LOC | Share |
|---|---|---|---|
| A | Reusable GPUI-free logic: `emoji.rs`, `format.rs`, `i18n.rs` (fluent, 9 locales, ~1,074 en-US keys, `crates/ui/locales/*`), `images.rs`, `lookup.rs`, `rights.rs`, `access.rs` and similar | 1,885 | 4.6 % |
| B | Mixed: `app.rs` event pump, `timeline.rs` diff/load/ack policy, `composer.rs` autocomplete/typing/drafts, message body → styled runs, theme loader, … About half is portable to a Rust bridge layer | 9,409 | 22.8 % |
| C | Pure GPUI views | ≈30,019 | 72.7 % |

- Much of the view-model already lives in `state` (`rows.rs`/`views.rs`), which the zgui bridge can reuse directly.

**Parity target** (`ui/src/shell/mod.rs:122-156`) [read]:
- login (token / QR / web)
- server rail with drag-reorder, channel and DM sidebars, quick switcher
- timeline with embeds, polls, reactions, stickers, spoilers, replies, edit-in-place, multi-select
- composer with autocomplete, attachments, paste and drafts
- emoji, GIF and sticker pickers
- member list (not virtualised, `member_list.rs:418`)
- friends, profiles, threads, forums, pins
- server settings (4.7k LOC), account settings (4.4k LOC, 20 pages), moderation
- voice and video UI (4.0k), plugin UI (4.2k), updates, theming, zoom

**Chat transcript placement**
- oxidcord uses GPUI's measured `ListState` with tail-follow and `splice`-based prepend, where anchoring comes from GPUI semantics (`ui/src/components/chat_list.rs:15-24,65-94`; `ui/src/shell/timeline.rs:216-234`) [read].
- zgui constraints (core.md A §4) [read @a9dc282]:
  - Luau `ui.list` takes only fixed or app-supplied heights, is index-keyed and has no follow (`crates/zgui-script/src/node.rs:753-773`).
  - Script nodes have **no host-view kind** (`node.rs:658-848`; only WebView, Terminal and a gated Library), so a Rust view can't sit *inside* a Luau tree.
- **Recommendation [inferred]:**
  - Build the transcript as a Rust view (`measured_rows` + `VariableHeights::anchor_end()`, as zgui's own `examples/zeron.rs:375,633` does) with hand-built prepend anchoring.
  - Place it **beside** `host.view()` in a Rust root layout. If the composer must sit under the transcript, use a second `ScriptHost`; this is untested.
  - Row overlays (context menu, emoji popover, profile card) call back into Luau through host actions.
  - Keep "Luau `ui.list` + Rust-computed heights" only as a spike comparison: every row type's layout would have to be reproduced exactly in Rust.

**Size and effort** [inferred]

| Scope | Luau | Rust glue | Effort |
|---|---|---|---|
| Full parity | ≈13k–22k LOC, from ≈27k GPUI view LOC at 0.5–0.8 Luau lines per GPUI line. Without voice: −2–3k | ≈5k–9.5k LOC: bridge, transcript, `media` → `ScriptHost::image` with an animation ticker, plugin/i18n/theme exposure | **14–26 engineer-weeks** |
| MVP (login, rail, sidebars, composer, member list, top bar, user panel, switcher, pickers, settings shell, components, friends, profile, lightbox) | ≈5.5k–9k LOC | same glue, smaller | **7–12 engineer-weeks** |

- For the Luau ratio, zgui's larger Luau apps are 1.5k–2.8k lines each (`composer_states/app.luau`, `agent_trace/app.luau`, `frame_profiler/app.luau @a9dc282`).

**Core work for path (c)** [inferred]

| Work item | Effort |
|---|---|
| session-lite without voice and plugins | 1–2 weeks |
| gateway fixes (v9, capability set, `client_state` versions, sliding priority window, op 37 ranges and debounce, op 8 scheduler) | 1–1.5 weeks |
| REST fingerprint and safety layer (cookie jar, per-channel Referer, timezone, full super-properties, reduced UA, installation id, probe limiter, pacing, 401 stop, 403 breaker, captcha → UI) | 1.5–2 weeks |
| state bounds and versions, member-list refresh, `lazy_private_channels`, ADD_MANY / PASSIVE_UPDATE (or drop those bits), persisted `client_state` | 2–3 weeks |
| safety locks, ack debounce, store and media fixes | 1–1.5 weeks |
| tests and recorded fixtures (none exist) | 1.5–2 weeks |
| validation on a throwaway real account | 0.5–1 week |
| **Total** | **≈8.5–13 weeks** |

- From scratch, core.md B §11 estimates 10–16 weeks.
- **The saving is modest on the risky parts. The real gain is breadth:** endpoints, models, permission math, markdown, media and lenient decoding, at an MIT/Apache licence.

**Total to a zgui MVP:** ≈**16–25 engineer-weeks**. That is the core work (8.5–13) plus the UI MVP (7–12), with the one-week Windows spike from core.md absorbed into the core. Full UI parity adds about 7–14 weeks more.

### Path comparison

| Path | Assessment |
|---|---|
| (a) Hard fork as-is | No. You'd inherit voice and wasmtime, the plugin automation surface, the unsafe defaults and zero tests, then fix them in place anyway |
| (b) Git dependency + contribute back | No. `session` can't be consumed without voice and plugins. Upstream rules (no tests, no comments, `AGENTS.md:7-8`) clash with what we need. It's one author with about 4 hours of public history, so churn is unknown. Upstreaming individual fixes as patches remains possible and polite |
| **(c) Vendor a subset (recommended)** | Vendor `protocol`, `markdown`, `media`, `gateway`, `rest`, `state`, `store`, `login` (token + QR) and `platform` (minus `single_instance`) into our repo as owned crates. Write session-lite. Run a correctness and safety pass against core.md Part B before any real-account use |
| (d) Reference only | The right treatment for `session`, `ui`, `voice` and `plugin-host` |

### Attribution obligations

oxidcord is dual-licensed MIT OR Apache-2.0 (`Cargo.toml` workspace `license`; `README.md:174-183`). [read; obligations inferred, not legal advice]

- Keep both `LICENSE-MIT` and `LICENSE-APACHE` texts, including "Copyright (c) 2026 Oxidcord contributors", with the vendored crates (or in a `THIRD_PARTY_NOTICES`).
- Under the Apache option:
  - carry `NOTICE` (`NOTICE:1-8`, Apache §4(d))
  - mark modified files prominently (§4(b))
- Under the MIT option: keep the copyright and permission notice. Our own code can stay MIT.
- Don't reuse the "Oxidcord" name or icon.
- `crates/protocol/src/expressions/unicode_table.rs` (1,923 emoji with shortcodes and keywords) gives no source. It looks like GitHub gemoji (MIT) [inferred, unverified]. Verify, and add that notice if so.
- `crates/ui/locales/*.ftl` are under the repo licence if reused.
- Updater and catalog keys (`crates/updater/src/keys.rs:1-4`; `crates/session/src/catalog.rs:16`) and GitHub endpoints are upstream's. Replace them or drop those crates.

---

## Sources

**oxidcord @ `497958b`** (`C:\Users\ADMIN\AppData\Local\Temp\oxidcord-src\oxidcord`, read only)
- Top level and docs: `AGENTS.md`, `README.md`, `NOTICE`, `LICENSE-*`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `docs/architecture.md`, `.github/workflows/{ci,release}.yml`, `git log`/`git show --stat`
- `crates/gateway/src/{config,error,event,frames,handle,limiter,session,transport}.rs`
- `crates/protocol/src/{gateway,rest,token,lenient,guild,snowflake,channel,message}.rs`
- `crates/rest/src/{config,error,route,request,limiter,client}.rs`, `api/{build,gateway,users,channels}.rs`
- `crates/login/src/{token_login,persist,error,web_login}.rs`, `qr/{mod,crypto,messages,flow}.rs`
- `crates/store/src/{lib,seal,token,atomic,paths,migrations,accounts,database,settings}.rs`
- `crates/session/src/{session,config,notifier,command,render,persist,broker,catalog,task}.rs`, `task/{commands,actions,plugin_link,voice_link}.rs`
- `crates/state/src/{state,changes,ready,dispatch,messages,member_list,permissions,notifications,rows,views}.rs`
- `crates/media/src/{decode,cdn,fetch,cache,service,types}.rs`
- `crates/markdown/src/{inline,block,link,entity,fence,document,ast}.rs`
- `crates/platform/src/*`, `crates/plugin-host/{wit/oxidcord.wit,src/slots.rs,src/host_state.rs}`, `crates/updater/src/{keys,net,manifest}.rs`, `crates/voice/Cargo.toml`
- `crates/ui/src/{app,data,images,i18n,desktop}.rs`, `shell/{mod,timeline,composer,member_list}.rs`, `shell/plugins/slot_renderer.rs`, `components/chat_list.rs`, `locales/*`
- `apps/desktop/{Cargo.toml,src/main.rs}`, `tools/mockcord/src/{gateway,ready,traffic,server}.rs`, `rest/mod.rs`, `tools/xtask/src/{main,headless,checks,release}.rs`

**zgui @ `a9dc282`** (`C:\Users\ADMIN\AppData\Local\Temp\zgui-src`, read only)
- `Cargo.toml`, `Cargo.lock`, `vendor/*/Cargo.toml`, `vendor/winit/src/platform_impl/windows/event_loop.rs`
- `crates/zgui-desktop/{Cargo.toml,src/application.rs,src/file_dialog.rs,src/native_host/webview2.rs}`
- `crates/zgui/src/{web_view,rich_text,image}.rs`, `crates/zgui-script/src/{node,lib}.rs`
- `docs/{linux,platform-services,getting-started}.md`, `examples/zeron.rs`, example `app.luau` files

**Our research:** `C:\Users\ADMIN\Desktop\nguyenvu\_research\zgui-discord-client\core.md` (Parts A and B and their sources)

**Community docs (fetched 2026-10-25)**
- https://docs.discord.food/gateway/using-gateway (capability bits 0–16 and their effects)
- https://docs.discord.food/topics/gateway-events (READY `auth_token`, Passive Update V2, Message Reaction Add Many, client_state fields, Identify fields)
- https://docs.discord.food/resources/message ("Bulk Delete Messages … is not usable by user accounts")
