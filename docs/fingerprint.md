# Client fingerprint

Kyoko presents itself as the Discord web client in Chrome, on both the gateway and REST. Everything that makes up that identity lives in `crates/protocol/src/fingerprint.rs`, plus the IDENTIFY builder in `crates/gateway/src/frames.rs`. When Chrome or Discord moves, update those two files. REST, the gateway and the QR socket also present Chrome's TLS and HTTP/2 handshake through `crates/net`; media and theme links do not yet. See "TLS and HTTP/2 handshake".

## What is sent

- **API v9** on REST (`/api/v9`) and the gateway (`v=9`).
- **User agent**: Chrome's reduced form, `Chrome/<CHROME_MAJOR>.0.0.0`. The same string goes in the gateway handshake, every REST request, the build-number scrape and the QR login socket.
- **Super-properties**: one `IdentifyProperties` object, in the web client's key order: `os`, `browser`, `device`, `system_locale`, `browser_user_agent`, `browser_version`, `os_version`, `referrer`, `referring_domain`, `referrer_current`, `referring_domain_current`, `release_channel`, `client_build_number`, `client_event_source`, `has_client_mods`, `client_launch_id`, `launch_signature`, `client_app_state`, `client_heartbeat_session_id`. The gateway sends it as IDENTIFY `properties`, with `installation_id` appended once kyoko knows one. REST sends the same JSON without `installation_id`, base64-encoded, as `X-Super-Properties`. Apart from `installation_id`, the two are identical at any moment.
- **Launch ids**: `client_launch_id` is a fresh UUID per launch. `launch_signature` is a fresh UUIDv4 per launch with the twelve client-mod detection bits (`LAUNCH_SIGNATURE_MOD_BITS`) cleared, so it says "no client mods" as `has_client_mods: false` does.
- **Heartbeat session id**: `client_heartbeat_session_id` starts as a fresh UUID and is replaced by a new one once it is 30 minutes old (`HEARTBEAT_SESSION_LIFETIME`). `rest::ClientIdentity` checks its age every time it hands out properties, for a REST request or an IDENTIFY, so gateway and REST always carry the same id. The age comes from a `fingerprint::Clock`, the system clock unless a test injects one.
- **Build number**: at startup the session fetches `https://discord.com/app`, then reads the build number from its scripts, sentry first. Discovery takes at most 5 s and IDENTIFY waits for it. On failure the last discovered build is used (`settings.client.build_number`), then `FALLBACK_BUILD_NUMBER`. This scrape is the only request outside the API. It goes only to discord.com or loopback.
- **REST headers**: `X-Discord-Locale`, `X-Discord-Timezone` (the system IANA zone), `Accept-Language`, Chrome client hints, `Sec-Fetch-*`, `Priority`, `Origin` on non-GET requests, and a `Referer` that follows the open channel (`/channels/<guild>/<channel>`, `/channels/@me/<channel>`, or `/channels/@me` before a channel is opened). The QR login's ticket exchange and the email form's `/auth/login`, `/auth/mfa/sms/send` and `/auth/mfa/<method>`, made from the login page, send `Referer: https://discord.com/login` (`LOGIN_REFERER`) and no `Authorization`.
- **Client hints**: `sec-ch-ua` is computed from `CHROME_MAJOR` the way Chrome computes it (below). For 156 it is `"Not:A-Brand";v="8", "Chromium";v="156", "Google Chrome";v="156"`.
- **Installation id**: taken from READY's `apex_experiments.installation` the first time it appears. It is stored and sent as `X-Installation-ID` and as IDENTIFY `properties.installation_id` from then on.
- **Context properties**: joining a server and sending a friend request from the Add Friend tab send `X-Context-Properties` (`{"location":"Add Friend"}` for the latter, `ContextProperties::relationship`). For a join it is base64 of the compact JSON `{"location","location_guild_id","location_channel_id","location_channel_type"}` (plus `location_message_id` for an invite in a message), in that order. The add-server dialog says `"Join Guild"` with the invite's server, channel and channel type; an invite shown in a message (`InviteSource::Message`, for invite cards in the timeline, not drawn yet) says `"Invite Button Embed"` with the message's channel and id (`fingerprint::ContextProperties`, `JoinLocation`, `rest::Request::context_properties`). No other request carries it yet (`docs/rest.md`, "Relationships"). The shape comes from the spike's notes and community research of the web client; userdoccers does not document the header, so check it on the next real capture.
- **Cookies**: one jar, filled from Discord's responses, plus `__dcfduid`, `__sdcfduid` and `_cfuvid` carried over from the web login page (`docs/sign-in.md`, "Web login").
- **Web login**: the login window is honestly WebView2. It sets no user agent, so the page sees WebView2's own (Edge, `Edg/<version>`) beside WebView2's own `Sec-CH-UA` and `navigator.userAgentData` brands (`Microsoft Edge WebView2`). WebView2 lets an app replace the user agent string but not the client hints, so kyoko's Chrome string there would sit beside Edge brands, a mismatch bot checks such as Cloudflare's flag. This is only the login page: once the token is handed over, REST, the gateway and the QR socket present the Chrome web client above, carrying the page's cookies.
- **Images**: CDN and media-proxy fetches send Chrome's `<img>` headers from `Fingerprint::image_headers`: the same user agent and client hints, an image `Accept`, `Referer: https://discord.com/`, `Sec-Fetch-Dest: image`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site` and `Priority: i`. They carry no cookies, because discord.com cookies are never sent to Discord's CDN hosts. See `docs/media.md`.

## TLS and HTTP/2 handshake

REST (the API, uploads, the build-number scrape and the email login) and the gateway run on `wreq`, which runs BoringSSL with Chrome's TLS and HTTP/2 options, through the headless `crates/net`. Media and theme links still use reqwest on rustls, and the QR socket tokio-tungstenite on rustls, so their ClientHello is a Rust program's until tickets 72 and 73 move them. Ticket 69 was the trial: `cargo xtask tls-capture` (`docs/testing.md`) recorded the local Chrome and a `wreq` client against a localhost listener, and nothing contacted Discord. Tickets 70 and 71 moved REST and the gateway; the session tests in `crates/session/tests/session/tls_fingerprint.rs` hold them to the recorded Chrome.

### Pins

`wreq-util` (rev `5715529`), `wreq` (rev `cd76bcd`) and `btls`/`btls-sys` (rev `1298875`) are git pins: no crates.io release has a Chrome profile newer than 149, and the newest at these revs is Chrome154. `wreq-util` is a normal dependency; `wreq`, `btls` and `btls-sys` are `[patch.crates-io]` entries, because `wreq-util`'s own patch section is ignored when it is a dependency. `emulation-chromium-pki` is on; without it the Chrome 152 to 154 profiles send an empty Trust Anchor IDs extension. `cargo tree -e features -i btls-sys` shows only its `default` feature, so neither `relax-cert-validation` nor `allow-crl-extensions-bad-version` is applied. Older `wreq-util` releases were GPL-3.0 or LGPL-3.0; the pinned rev is Apache-2.0 and `cargo deny check` guards it. Its `emulation` feature also pulls zstd 0.13 and zstd-safe 7, which have advisories (RUSTSEC-2026-0391, -0398, -0403) fixed only in zstd-safe 8. wreq-util uses zstd only for the certificate compressor of its Firefox profiles; the Chrome profile offers brotli only, so `deny.toml` ignores the three with that reason. A wreq-util rev that moves to zstd 0.14 should drop the ignores.

`net::profile(major)` maps a Chrome major to the newest wreq-util profile whose handshake was captured equal: 154 and 156 map to `Profile::Chrome154`. `net::CHROME_PROFILE` is `profile(CHROME_MAJOR)`, evaluated at compile time, so bumping `CHROME_MAJOR` to a major with no captured profile fails the build (see "Bumping Chrome" below). `net::chrome_emulation(destination)` takes that profile and clears its headers and original-header map, because the profile's headers are a navigation's (`upgrade-insecure-requests`, `sec-fetch-user: ?1`, `accept: text/html...`) and would leak into kyoko's requests. Kyoko stays the only source of headers. `Destination::Fetch` sets the HEADERS priority weight to 220, as Chrome's `fetch()`; `Destination::Document` keeps the profile's 256, as a page load. `net::websocket_emulation()` is the fetch emulation with `alps_protocols` removed, for websocket requests (below).

`net::client_builder(destination, trust)` builds a client from that emulation, with no proxy. `net::Trust::Bundled` (the default) keeps wreq's bundled roots; `Trust::Only(roots)` trusts only the given PEM roots. Only tests use it, through `SessionConfig.trust`, `RestConfig.trust`, `GatewayConfig.trust` and `QrConfig.trust`. Certificate checks are never turned off.

Who uses what:

- **API and uploads** (`RestClient`): `Destination::Fetch`, so every API request goes out with weight 220, as the web client's `fetch()` calls do. Kyoko's default headers, no redirects, kyoko's `CookieJar`.
- **Build-number scrape** (`rest::api::build::discovery_client`): `Destination::Document`, weight 256, as the `/app` page load. Its script fetches share the client, so they carry 256 too, where Chrome gives scripts a lower weight; it runs once per launch.
- **Gateway**: one client per gateway, `Destination::Fetch`, with the websocket emulation on each upgrade (`net::ws::connect`). REST and the gateway build separate clients from the same function. They talk to different hosts (discord.com and gateway.discord.gg), so Chrome would not share a connection or a TLS session between them either.
- **QR socket** (`login::qr`): its own client per attempt, `Destination::Fetch`, with the websocket emulation on the upgrade, as the gateway (remote-auth-gateway.discord.gg is another host again). Its ticket exchange is a `RestClient` like the API's.
- **Cookies**: `rest::CookieJar` implements wreq's `CookieStore`. On HTTP/2 it returns one `cookie` field per cookie, as Chrome does; on HTTP/1.1 one `Cookie` line. It persists, restores and adopts the web login page's cookies as before.

### What Chrome 156 sends

Recorded from Chrome 156.0.8078.48 on Windows, fresh profile, on 2026-10-27 (`tools/mockcord/fixtures/tls/chrome-156-https.json` and `chrome-156-wss.json`).

- **https** (JA4 `t13d1517h2_8daaf6152771_cb7bf5808d99`): GREASE then the 15 TLS 1.3, ECDHE and legacy RSA suites (`TLS_AES_128_GCM_SHA256` first, `TLS_RSA_WITH_AES_256_CBC_SHA` last). 17 extensions plus a GREASE extension first and last, in a new order each connection: server name, status request, supported groups, EC point formats, signature algorithms, ALPN, SCT, extended master secret, certificate compression (brotli), empty session ticket, supported versions (GREASE, 1.3, 1.2), PSK modes (`psk_dhe_ke`), key share, ALPS on the new codepoint (17613) for `h2`, Trust Anchor IDs (28 IDs, sorted by their bytes), ECH GREASE (HKDF-SHA256, AES-128-GCM, 32-byte enc) and renegotiation info. Groups GREASE, X25519MLKEM768, X25519, P-256, P-384, with key shares for GREASE (1 byte), X25519MLKEM768 and X25519. Signature algorithms GREASE, ML-DSA-44, -65, -87, then the usual eight ECDSA, RSA-PSS and PKCS#1 ones. ALPN `h2`, `http/1.1`. No pre-shared key on a fresh connection.
- **wss** (JA4 `t13d1516h1_8daaf6152771_5b37b36a67c0`): the same ClientHello on its own connection, except that ALPN is `http/1.1` only and there is **no ALPS** extension. The upgrade request is HTTP/1.1 with title-case names in this order: `Host`, `Connection: Upgrade`, `Pragma: no-cache`, `Cache-Control: no-cache`, `User-Agent`, `Upgrade: websocket`, `Origin`, `Sec-WebSocket-Version: 13`, `Accept-Encoding: gzip, deflate, br, zstd`, `Accept-Language`, `Cookie` (one line), `Sec-WebSocket-Key`, `Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits`.
- **HTTP/2** (Akamai `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`): the first flight is SETTINGS (header table 65536, push off, initial window 6291456, max header list 262144, in that order), a connection WINDOW_UPDATE of 15663105, then HEADERS. No PRIORITY frames. Pseudo-headers `:method`, `:authority`, `:scheme`, `:path`. Every HEADERS frame carries END_HEADERS and a priority: the navigation is exclusive on stream 0 with weight 256; `fetch()` and image requests use weight 220 (`priority: u=1, i`), and a later request on the same connection depended exclusively on the earlier one (stream 3 on stream 1). Cookies are split: each cookie is its own `cookie` field.
- Header order of a same-origin `fetch()` on HTTP/2: `sec-ch-ua-platform`, `user-agent`, `sec-ch-ua`, `sec-ch-ua-mobile`, `accept`, `sec-fetch-site`, `sec-fetch-mode`, `sec-fetch-dest`, `referer`, `accept-encoding`, `accept-language`, `cookie`..., `priority`.

The capture ran Chrome with `--disable-background-networking` and a new profile, so no variations seed was loaded: field trials that change TLS behaviour, if any are running, are at their defaults.

### What wreq's Chrome154 sends, and the difference

`cargo xtask tls-capture --client wreq --h2 --compare chrome-156` (fixtures `wreq-chrome154-https.json` and `-wss.json`) reports **no difference** for https or wss: same JA4s, cipher list, extension set, GREASE places (first and last extension), groups, key shares, signature algorithms, ALPN, ALPS, certificate compression, ECH GREASE parameters, the same 28 Trust Anchor IDs in the same order, SETTINGS, WINDOW_UPDATE, first HEADERS priority and pseudo-header order. Two things were needed for that:

- **Websocket ALPS.** wreq offers ALPN `http/1.1` only for a websocket, but still adds the ALPS extension for `h2` from the profile's `TlsOptions`, so its wss hello had 17 extensions (`t13d1517h1_...`) where Chrome has 16. Per request, `client.websocket(url).emulation(net::websocket_emulation())` drops ALPS and the hello matches. This is the one `TlsOptions` override kyoko needs.
- **SETTINGS ACK timing.** With a listener that sends its SETTINGS the instant TLS completes, wreq acknowledged them before sending its WINDOW_UPDATE. With 50 ms of latency, as any real network has, its first flight is SETTINGS, WINDOW_UPDATE, HEADERS like Chrome's. The listener now waits 50 ms.

Trusting a test root through `tls_cert_store` instead of turning certificate checks off leaves the ClientHello unchanged (`--trust-root --compare wreq-chrome154`: no difference). The trust-anchor list comes from the profile, not from the certificate store.

Left over, outside what the comparison checks:

- **Priority chaining.** API requests carry Chrome's `fetch()` weight, 220, exclusive on stream 0. Chrome makes a later request on the same connection depend exclusively on the one before; wreq cannot chain, so every kyoko request depends on stream 0.
- **Header order and case.** wreq sends the headers kyoko gives it, in kyoko's order: the default headers first (`user-agent`, `accept`, `accept-encoding`, `accept-language`, `x-discord-locale`, `x-discord-timezone`, the client hints, `sec-fetch-*`, `priority`), then the per-request ones (`authorization`, `x-super-properties`, `x-installation-id`, `referer`, `origin`), then the `cookie` fields. Chrome's order is above. On the websocket upgrade wreq writes lowercase names in its own order (`sec-websocket-version`, `upgrade`, `connection`, `sec-websocket-key`, `accept-encoding`, `host`, `cookie`) and no `Pragma`, `Cache-Control` or `Sec-WebSocket-Extensions`; `OrigHeaderMap` can set case and order, but permessage-deflate cannot be offered without supporting it. Ticket 74 decides on both.
- **Image `Accept`.** Chrome 156 sends `image/jxl,image/avif,...` for images; `fingerprint::IMAGE_ACCEPT` has no `image/jxl` (ticket 74).
- Not covered by a local capture: HTTP/3 after Cloudflare's `alt-svc`, real ECH from DNS HTTPS records, and session resumption.

### Recommendation

Keep claiming Chrome 156 (`CHROME_MAJOR = 156`) and use wreq-util's Chrome154 TLS and HTTP/2 options for it, with the emulation headers cleared. Chrome 156's handshake equals Chrome154's in every field the capture compares, so no `TlsOptions` override is needed for https and kyoko does not need to claim 154. Websockets need the ALPS override above. Re-run the capture when Chrome moves to 157 (`cargo xtask tls-capture --client chrome --h2 --record chrome-157`, then `--client wreq --compare chrome-157`).

### Bumping Chrome

1. Update the local Chrome and record it: `cargo xtask tls-capture --client chrome --h2 --record chrome-<major>`.
2. Compare wreq's profile with it: `cargo xtask tls-capture --client wreq --h2 --compare chrome-<major>`.
3. With no difference, add the major to `net::profile` beside the profile it equals. With a difference, bump the wreq-util, wreq and btls revs to a profile that matches, or set the differing `TlsOptions` or `Http2Options` fields in `crates/net/src/chrome.rs`.
4. Bump `CHROME_MAJOR`. The build fails until step 3 maps it, and the `tls_fingerprint` session tests fail until `chrome-<major>-https.json` and `chrome-<major>-wss.json` exist and match.

### Build cost

Measured on the development machine (i9-12900HK, `CARGO_BUILD_JOBS=3`, sccache as `RUSTC_WRAPPER`, other worktrees building at the same time), 2026-10-27:

- **First build of `btls-sys`**, the build script that runs CMake, MSBuild and bindgen: 383 s for dev (RelWithDebInfo, because dependencies build at `opt-level = 3` with debug info) and 201 s for release. The wreq, btls and http2 Rust crates add about 15 s. A changed `BORING_BSSL_*`, `CMAKE*` or `HOST_CMAKE_TOOLCHAIN_FILE` value reruns it (about 4 min with the CMake build dir already there).
- **No-op rebuild** of `kyoko-net`: 0.9 s.
- **sccache** caches none of the C build: its C/C++ miss count did not move during it. CMake's Visual Studio generator ignores the compiler launcher that `btls-sys` passes.
- **Git fetch**: `~/.cargo/git` grew by 672 MB, mostly the BoringSSL submodule (192 MB database, 455 MB `btls` checkout with it).
- **Target dir**: about 0.6 GB per profile (dev: 337 MB `btls-sys` build dirs, a 252 MB patched copy of BoringSSL among them, 186 MB of new crates, 69 MB for the probe example; release: 301 MB of `btls-sys` build dirs).
- **Release `kyoko.exe`**: 78,949,888 bytes today, 83,832,832 bytes (+4.66 MiB, +6.2 %) with a temporary call into `crates/net` (client build, a GET and a websocket open), while reqwest, rustls and aws-lc are still linked. Ticket 74 removes those.

Native Windows builds of `btls-sys` assemble BoringSSL's assembly unless they cross-compile, which needs NASM. `.cargo/config.toml` points `HOST_CMAKE_TOOLCHAIN_FILE` at `.cargo/boringssl-no-asm.cmake`, which sets `OPENSSL_NO_ASM` (and the release C runtime, which `btls-sys` stops setting when a toolchain file is given). The cost is slower bulk crypto; the TLS 1.3 cipher order is unchanged, because the profile sets `aes_hw_override`, as the capture shows. libclang was found in `C:\Program Files\LLVM` without `LIBCLANG_PATH`.

**Sharing one BoringSSL between worktrees.** `BORING_BSSL_INSTALL_DIR` does not work on Windows at this rev: it copies `libcrypto.a` and `libssl.a`, and MSVC builds `crypto.lib` and `ssl.lib`, so the build script fails. Copying by hand works: after a release build, copy `target/release/build/btls-sys-*/out/build/Release/{crypto,ssl}.lib` into `<dir>/lib` and `target/release/build/btls-sys-*/out/boringssl/include` into `<dir>/include`, outside the repo, one dir per `btls` rev. Then set `BORING_BSSL_PATH=<dir>` and `BORING_BSSL_ASSUME_PATCHED=1`: the build script runs bindgen only (2.6 s), CMake is not needed, and the capture still shows no difference from Chrome 156. Release libraries link into dev builds too (both use the `/MD` runtime).

**CI.** No workflow file changed. `windows-latest` has CMake on `PATH`, LLVM in `C:\Program Files\LLVM` and Git, and the toolchain file removes the need for NASM, so `ci.yml` should build and test the workspace as it is, with the first run paying the BoringSSL fetch and build (`Swatinem/rust-cache` keeps `~/.cargo/git` and `target` afterwards). `release.yml` builds `-p kyoko`, which links `crates/net` since ticket 70, so its Windows job pays the BoringSSL build too. Its ubuntu `publish` job builds `xtask`, which does not depend on `crates/net`, so it builds no BoringSSL, but cargo still fetches the git pins and the BoringSSL submodule to load the workspace. None of this has run on CI yet.

## IDENTIFY

Keys in this order: `token`, `properties`, `compress` (`false`), `presence`, `capabilities`, `client_state`. There is no `large_threshold`; the user default is already 250.

`presence` is `{"activities":[],"status":"unknown","since":0,"afk":false}`, as the docs say a user client sends on a fresh IDENTIFY. Discord then applies the status from the user's settings.

`client_state` always carries `api_code_version`: READY's value once kyoko has seen one, `0` before that. See `docs/client-state.md`.

## Chrome major and the brand list

`CHROME_MAJOR` is 156, Chrome's stable major on 2026-10-25. Per [Chromium Dash](https://chromiumdash.appspot.com/schedule) (`fetch_milestone_schedule`, read 2026-10-25), M156 reached stable on 2026-10-20. M157 is in early stable only (157.0.8103.15, released 2026-10-21), with full stable on 2026-11-03. Move to 157 after that date.

Chrome derives its GREASE brand and brand order from the major version, so bumping the major changes the whole `sec-ch-ua` value. Kyoko computes it the same way in `brand_list`. The source is `GenerateBrandVersionList`, `GetGreasedUserAgentBrandVersion` and `ShuffleBrandList` in Chromium's [`components/embedder_support/user_agent_utils.cc`](https://chromium.googlesource.com/chromium/src/+/main/components/embedder_support/user_agent_utils.cc), read at main on 2026-10-25, which follows the [UA-CH spec's arbitrary brands section](https://wicg.github.io/ua-client-hints/#create-arbitrary-brands-section). With `seed` = major:

- The GREASE brand is `"Not" + c[seed % 11] + "A" + c[(seed + 1) % 11] + "Brand"`, where `c` is `' ' ( : - . / ) ; = ? _`. Its version is `["8", "99", "24"][seed % 3]`.
- The list is `[GREASE, Chromium, Google Chrome]`. It is permuted by `orders[seed % 6]`, where `orders` is `012, 021, 102, 120, 201, 210`: item `i` goes to position `order[i]`.

Known values check out: 120 gives `"Not_A Brand";v="8"` first, 131 gives `"Google Chrome", "Chromium", "Not_A Brand";v="24"`, and 134 gives `"Chromium", "Not:A-Brand";v="24", "Google Chrome"`. The old hard-coded value, `"Not/A)Brand";v="99"` in the middle, was Chrome 115's and was wrong for 152.

## Sources

The web client's identity follows discord-userdoccers, served as docs.discord.food, read at master (`01624d9`) on 2026-10-25:

- [Client properties](https://docs.discord.food/reference#client-properties), page last changed 2026-08-18: the field list, the web example and its key order, `client_heartbeat_session_id` "regenerated every 30 minutes", and footnote 3, which says `installation_id` is sent only when identifying and is not in `X-Super-Properties`.
- [Launch signature](https://docs.discord.food/reference#launch-signature): the mod-detection bit table and a generator for a default signature (a random UUIDv4 with those bits forced to 0). Kyoko implements that generator. No GPL client code was read for it.
- [Identify](https://docs.discord.food/gateway/gateway-events#identify), page last changed 2026-08-27: the key order of the example, presence `unknown` with `since: 0`, and `client_state.api_code_version` (`0` in the example).
- [Installations](https://docs.discord.food/topics/experiments#installations): send a known installation id as `X-Installation-ID` and as `installation_id` when identifying.

Not adopted:

- Op 41 (Update Time Spent Session ID), which the docs recommend sending on connect and every 30 minutes, isn't sent. It is optional and wasn't in scope.
- `client_app_state` isn't in the docs' web example. Kyoko keeps sending `"focused"` until a real capture says otherwise. Its position, after `launch_signature`, is a guess.
- `search_engine_current` and `mp_keyword_current` belong to search referrals, which a native client never has.

## GIF provider

Discord moved its GIF picker from Tenor to Klipy in 2026 (Google closed the Tenor API on 2026-06-30). Kyoko keeps the provider in one constant, `rest::api::GIF_PROVIDER` in `crates/rest/src/api/expressions.rs`: `query` is the `provider` value sent to the `/gifs/*` endpoints (`None` sends no parameter) and `name` is the label in the picker's search field ("Search Klipy").

It is set to `query: None`, `name: "Klipy"`. Evidence, read 2026-10-26 with a neutral `kyoko-dev` user agent, no request to Discord:

- discord-userdoccers commit [`17502f2`](https://github.com/discord-userdoccers/discord-userdoccers/commit/17502f2edbb907e4db2a9e8b7539a6912c205f47) "Readd providers to GIF APIs" (2026-01-15) added an optional `provider` to `/gifs/search`, `/gifs/trending`, `/gifs/trending-gifs`, `/gifs/suggest`, `/gifs/trending-search` and the `POST /gifs/select` body, "The GIF provider to use (default `tenor`)", with the values `tenor`, `giphy` and `klipy`.
- Commit [`7927788`](https://github.com/discord-userdoccers/discord-userdoccers/commit/7927788f83e9dde50bb7827e1c5095e525451306) "Remove deprecated gif APIs" (2026-07-15) removed the `provider` parameter and the provider table from every GIF endpoint, and marked `/gifs/suggest` and `/gifs/trending-search` deprecated. The current [integration page](https://docs.discord.food/resources/integration) lists no provider, and an invalid `media_format` now falls back to `mp4` for every request.
- Press reports: PiunikaWeb (2026-01-14) on Discord testing Giphy and Klipy after Tenor stopped issuing API keys; PCGamesN (2026-06-30): the picker's box "now says 'Search Klipy'".

So the web client most likely sends no `provider` at all, with Klipy as the only backend; if it still sends one, the value is `klipy`. Kyoko also stopped calling `/gifs/suggest` from the picker. Still to confirm on a capture of the live web client (the orchestrator arranges it; never record the token): whether any `provider=` is sent and with what value, whether `/gifs/trending-gifs` carries a `limit`, and which hosts `src`, `gif_src` and `preview` now point to (the media allowlist already has `static.klipy.com`, `media.tenor.com` and `c.tenor.com`). Changing the answer is a one-line edit of `GIF_PROVIDER`; `session/tests/session/expressions.rs` (`gif_search_asks_discord_the_way_the_web_client_does`) pins the parameters.

## Sharing and persistence

`rest::ClientIdentity` holds the fingerprint, the cookie jar, the installation id and the clock that ages the heartbeat session. Build one per launch with `session::client_identity(&store)` (`client_identity_with_clock` in tests), then pass it to login (`TokenLogin::validate`, `QrConfig::identity`, `session::login`) and to `SessionConfig::identity`. That way every request shares one jar and one id. The gateway reads IDENTIFY `properties` from the same identity through a `gateway::PropertiesSource` on every connect, so a re-identify carries the current build, heartbeat session id and installation id. The installation id and the jar are stored in the encrypted `kv` table. The session saves the jar whenever it changes and again on shutdown.

`SessionConfig::build_source` is the page to scrape: `session::discord_build_source()` against Discord, `Server::app_url()` against mockcord, or `None` to skip discovery.
