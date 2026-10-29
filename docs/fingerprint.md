# Client fingerprint

Kyoko presents itself as the Discord web client in Chrome, on both the gateway and REST. Everything that makes up that identity lives in `crates/protocol/src/fingerprint.rs`, plus the IDENTIFY builder in `crates/gateway/src/frames.rs`. When Chrome or Discord moves, update those two files. REST, the gateway, the QR socket, media and theme links also present Chrome's TLS and HTTP/2 handshake through `crates/net`. See "TLS and HTTP/2 handshake".

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
- **Images**: CDN and media-proxy fetches send Chrome's `<img>` headers from `Fingerprint::image_headers`: the same user agent and client hints, an image `Accept`, `Referer: https://discord.com/`, `Sec-Fetch-Dest: image`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site`, `Sec-Fetch-Storage-Access: active` and `Priority: i`, in Chrome's order ("Header order" below). They carry no cookies, because discord.com cookies are never sent to Discord's CDN hosts. See `docs/media.md`.

## TLS and HTTP/2 handshake

REST (the API, uploads, the build-number scrape and the email login), the gateway, the QR socket, media (CDN and GIF images, audio) and theme links run on `wreq`, which runs BoringSSL with Chrome's TLS and HTTP/2 options, through the headless `crates/net`. BoringSSL is the app's only TLS library: since tickets 72 and 73, `cargo tree -p kyoko -e normal` has no reqwest, rustls or aws-lc-sys (tests and xtask still use them). Ticket 69 was the trial: `cargo xtask tls-capture` (`docs/testing.md`) recorded the local Chrome and a `wreq` client against a localhost listener, and nothing contacted Discord. Tickets 70 and 71 moved REST and the gateway; the session tests in `crates/session/tests/session/tls_fingerprint.rs` hold them to the recorded Chrome.

### Pins

`wreq-util` (rev `5715529`), `wreq` (rev `cd76bcd`) and `btls`/`btls-sys` (rev `1298875`) are git pins: no crates.io release has a Chrome profile newer than 149, and the newest at these revs is Chrome154. `wreq-util` is a normal dependency; `wreq`, `btls` and `btls-sys` are `[patch.crates-io]` entries, because `wreq-util`'s own patch section is ignored when it is a dependency. `emulation-chromium-pki` is on; without it the Chrome 152 to 154 profiles send an empty Trust Anchor IDs extension. `cargo tree -e features -i btls-sys` shows only its `default` feature, so neither `relax-cert-validation` nor `allow-crl-extensions-bad-version` is applied. Older `wreq-util` releases were GPL-3.0 or LGPL-3.0; the pinned rev is Apache-2.0 and `cargo deny check` guards it. Its `emulation` feature also pulls zstd 0.13 and zstd-safe 7, which have advisories (RUSTSEC-2026-0391, -0398, -0403) fixed only in zstd-safe 8. wreq-util uses zstd only for the certificate compressor of its Firefox profiles; the Chrome profile offers brotli only, so `deny.toml` ignores the three with that reason. A wreq-util rev that moves to zstd 0.14 should drop the ignores.

`net::profile(major)` maps a Chrome major to the newest wreq-util profile whose handshake was captured equal: 154 and 156 map to `Profile::Chrome154`. `net::CHROME_PROFILE` is `profile(CHROME_MAJOR)`, evaluated at compile time, so bumping `CHROME_MAJOR` to a major with no captured profile fails the build (see "Bumping Chrome" below). `net::chrome_emulation(destination)` takes that profile and clears its headers, because the profile's headers are a navigation's (`upgrade-insecure-requests`, `sec-fetch-user: ?1`, `accept: text/html...`) and would leak into kyoko's requests. Kyoko stays the only source of headers. It replaces the profile's original-header map with Chrome's recorded order for the destination: `net::API_ORDER` for `Destination::Fetch`, `net::IMAGE_ORDER` for `Destination::Image`, none for `Destination::Document` ("Header order" below). `Destination::Fetch` sets the HEADERS priority weight to 220, as Chrome's `fetch()`; `Destination::Image` sets 147, as Chrome's `<img>` with `priority: i`; `Destination::Document` keeps the profile's 256, as a page load. `net::websocket_emulation()` is the fetch emulation with `alps_protocols` removed and `net::UPGRADE_ORDER` as its header order, for websocket requests (below).

`net::client_builder(destination, trust)` builds a client from that emulation, with no proxy. `net::Trust::Bundled` (the default) keeps wreq's bundled roots; `Trust::Only(roots)` trusts only the given PEM roots. Only tests use it, through `SessionConfig.trust`, `RestConfig.trust`, `GatewayConfig.trust` and `QrConfig.trust`. Certificate checks are never turned off.

Who uses what:

- **API and uploads** (`RestClient`): `Destination::Fetch`, so every API request goes out with weight 220, as the web client's `fetch()` calls do, and its headers in `API_ORDER`. Kyoko's default headers, no redirects, kyoko's `CookieJar`.
- **Build-number scrape** (`rest::api::build::discovery_client`): `Destination::Document`, weight 256, as the `/app` page load. Its script fetches share the client, so they carry 256 too, where Chrome gives scripts a lower weight; it runs once per launch.
- **Gateway**: one client per gateway, `Destination::Fetch`, with the websocket emulation on each upgrade (`net::ws::connect`, which adds Chrome's `Pragma`, `Cache-Control` and `Accept-Encoding` to the caller's headers). REST and the gateway build separate clients from the same function. They talk to different hosts (discord.com and gateway.discord.gg), so Chrome would not share a connection or a TLS session between them either.
- **QR socket** (`login::qr`): its own client per attempt, `Destination::Fetch`, with the websocket emulation on the upgrade, as the gateway (remote-auth-gateway.discord.gg is another host again). Its ticket exchange is a `RestClient` like the API's.
- **Media** (`media::MediaFetcher`): one client, `Destination::Image`, kyoko's image, audio or video headers per request in `IMAGE_ORDER`, no cookies, no Authorization. Audio and video share the client, so they also go out with weight 147, which is what Chrome gives a `<video>` (`chrome-156-video.json`); Chrome's `<audio>` was not captured. The bridge test `crates/bridge/tests/bridge/media_fetch.rs` holds its ClientHello to the recorded Chrome. Like Chrome, it opens one connection per CDN host and multiplexes up to 100 image requests over it (`docs/media.md`, "Fetching and decoding"); `media_queue::a_burst_of_images_to_one_host_shares_one_http2_connection` sends ten at once from cold and finds one TLS connection carrying all ten at weight 147.
- **Theme links** (`session::theme_links`): a client per refresh, `Destination::Fetch` (so `API_ORDER`, which only places `user-agent` here), only the user agent, no cookies. Tests trust their own root through `ThemeLinkTuning.trusted_roots`, which becomes a `Trust::Only` (a wreq `CertStore`).
- **Cookies**: `rest::CookieJar` implements wreq's `CookieStore`. On HTTP/2 it returns one `cookie` field per cookie, as Chrome does; on HTTP/1.1 one `Cookie` line. It persists, restores and adopts the web login page's cookies as before.

### What Chrome 156 sends

Recorded from Chrome 156.0.8078.48 on Windows, fresh profile, on 2026-10-27 (`tools/mockcord/fixtures/tls/chrome-156-https.json`, `-wss.json`, `-image.json` and `-wss-cross-site.json`; `-video.json` was added on 2026-10-29 by v1 91 from the same Chrome; `docs/testing.md`, `tls-capture`).

- **https** (JA4 `t13d1517h2_8daaf6152771_cb7bf5808d99`): GREASE then the 15 TLS 1.3, ECDHE and legacy RSA suites (`TLS_AES_128_GCM_SHA256` first, `TLS_RSA_WITH_AES_256_CBC_SHA` last). 17 extensions plus a GREASE extension first and last, in a new order each connection: server name, status request, supported groups, EC point formats, signature algorithms, ALPN, SCT, extended master secret, certificate compression (brotli), empty session ticket, supported versions (GREASE, 1.3, 1.2), PSK modes (`psk_dhe_ke`), key share, ALPS on the new codepoint (17613) for `h2`, Trust Anchor IDs (28 IDs, sorted by their bytes), ECH GREASE (HKDF-SHA256, AES-128-GCM, 32-byte enc) and renegotiation info. Groups GREASE, X25519MLKEM768, X25519, P-256, P-384, with key shares for GREASE (1 byte), X25519MLKEM768 and X25519. Signature algorithms GREASE, ML-DSA-44, -65, -87, then the usual eight ECDSA, RSA-PSS and PKCS#1 ones. ALPN `h2`, `http/1.1`. No pre-shared key on a fresh connection. The image host's connection (`-image.json`) has the same hello with its own server name.
- **wss** (JA4 `t13d1516h1_8daaf6152771_5b37b36a67c0`): the same ClientHello on its own connection, except that ALPN is `http/1.1` only and there is **no ALPS** extension. The upgrade request is HTTP/1.1 with title-case names in this order: `Host`, `Connection: Upgrade`, `Pragma: no-cache`, `Cache-Control: no-cache`, `User-Agent`, `Upgrade: websocket`, `Origin`, `Sec-WebSocket-Version: 13`, `Accept-Encoding: gzip, deflate, br, zstd`, `Accept-Language`, `Cookie` (one line), `Sec-WebSocket-Key`, `Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits`. To another site (`-wss-cross-site.json`, as discord.com to gateway.discord.gg) it is the same list without `Cookie`.
- **HTTP/2** (Akamai `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`): the first flight is SETTINGS (header table 65536, push off, initial window 6291456, max header list 262144, in that order), a connection WINDOW_UPDATE of 15663105, then HEADERS. No PRIORITY frames. Pseudo-headers `:method`, `:authority`, `:scheme`, `:path`. Every HEADERS frame carries END_HEADERS and a priority: the navigation is exclusive on stream 0 with weight 256; `fetch()` (GET and POST alike) uses weight 220 (`priority: u=1, i`); the cross-site `<img>` uses weight 147 (`priority: i`, urgency 3). A request made while an earlier one on the connection is still open depends exclusively on it (the favicon on the API GET); one made after the earlier ones finished depends on stream 0. Cookies are split: each cookie is its own `cookie` field. A POST's HEADERS frame has no END_STREAM, and its `content-length` comes first, straight after the pseudo-headers.
- **API calls**: the page calls `fetch()` on its own origin, as the web client calls `discord.com/api`, with Discord's header names and dummy values (`mockcord::hello::DUMMY_API_HEADERS` and `API_CALLS`): a GET with `Authorization`, `X-Debug-Options`, `X-Discord-Locale`, `X-Discord-Timezone`, `X-Installation-ID` and `X-Super-Properties`; a POST adding `Content-Type: application/json`; a join-style POST adding `X-Context-Properties`; a PATCH adding `X-Audit-Log-Reason`. Chrome sends those headers neither in the page's order nor alphabetically. All four requests, and two runs of the capture, fit one fixed ranking (below), and reversing the order the page set them in changed nothing. That looks like a hash table's order, so a header set not captured here (several optional headers at once, or another Chrome major) may come out in another order: re-capture rather than guess.
- **Image**: an `<img>` on the page loading from another site (`kyoko-cdn.test`, as discord.com loads from cdn.discordapp.com) goes on its own connection with `accept: image/jxl,image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8`, `sec-fetch-site: cross-site`, `sec-fetch-mode: no-cors`, `sec-fetch-dest: image`, `sec-fetch-storage-access: active`, the page's origin as `referer`, no cookie and `priority: i` (weight 147). An earlier version of this file said `u=1, i`; the fixture has always said `i`.
- **Video**: a muted `<video preload=auto>` loading from a third site (`kyoko-video.test`, as discord.com loads from cdn.discordapp.com or media.discordapp.net) goes on its own connection with the `<img>`'s order and values except `accept: */*`, `accept-encoding: identity` (not the `identity;q=1, *;q=0` kyoko's audio guess sends), `sec-fetch-dest: video` and `range: bytes=0-` placed between `accept-language` and `priority`; `priority: i`, weight 147, exclusive on stream 0. The capture page's listener answers 404, so only the first request was seen; Chrome asks for later ranges as `bytes=N-` the same way. `Fingerprint::video_headers` and the `range` slot in `IMAGE_ORDER` copy it.

The capture ran Chrome with `--disable-background-networking` and a new profile, so no variations seed was loaded: field trials that change TLS behaviour, if any are running, are at their defaults.

### What wreq's Chrome154 sends, and the difference

`cargo xtask tls-capture --client wreq --h2 --compare chrome-156` (fixtures `wreq-chrome154-https.json` and `-wss.json`) reports **no difference** for https or wss: same JA4s, cipher list, extension set, GREASE places (first and last extension), groups, key shares, signature algorithms, ALPN, ALPS, certificate compression, ECH GREASE parameters, the same 28 Trust Anchor IDs in the same order, SETTINGS, WINDOW_UPDATE, first HEADERS priority and pseudo-header order. Two things were needed for that:

- **Websocket ALPS.** wreq offers ALPN `http/1.1` only for a websocket, but still adds the ALPS extension for `h2` from the profile's `TlsOptions`, so its wss hello had 17 extensions (`t13d1517h1_...`) where Chrome has 16. Per request, `client.websocket(url).emulation(net::websocket_emulation())` drops ALPS and the hello matches. This is the one `TlsOptions` override kyoko needs.
- **SETTINGS ACK timing.** With a listener that sends its SETTINGS the instant TLS completes, wreq acknowledged them before sending its WINDOW_UPDATE. With 50 ms of latency, as any real network has, its first flight is SETTINGS, WINDOW_UPDATE, HEADERS like Chrome's. The listener now waits 50 ms.

Trusting a test root through `tls_cert_store` instead of turning certificate checks off leaves the ClientHello unchanged (`--trust-root --compare wreq-chrome154`: no difference). The trust-anchor list comes from the profile, not from the certificate store.

Left over, outside what the comparison checks:

- **Priority chaining.** API requests carry Chrome's `fetch()` weight, 220, exclusive on stream 0. Chrome chains a request onto one still open on the same connection; wreq cannot, so every kyoko request depends on stream 0. That matches Chrome whenever kyoko's requests don't overlap. Image requests to one CDN host do overlap on its connection (up to 6 at once before ticket 93, up to 100 since), so a burst of images shows this gap: Chrome would chain each HEADERS onto the previous open stream, kyoko's all depend on stream 0 at weight 147.
- **Header order and case**: the next section.
- **Image `sec-fetch-storage-access` and `priority`.** Kyoko copies the capture: `sec-fetch-storage-access: active`, `priority: i` and weight 147. In Chrome the storage-access value follows the browser's third-party cookie state, and an image's priority depends on whether it is in view when the request starts, so a real page can send other values for some images. Re-check both at each bump.
- Not covered by a local capture: HTTP/3 after Cloudflare's `alt-svc`, real ECH from DNS HTTPS records, and session resumption.

### Header order

Kyoko sends Chrome's recorded header order (the user's decision on ticket 74). The lists live in `crates/net/src/order.rs`, copied by hand from the fixtures, one name per line:

- `API_ORDER`, for `Destination::Fetch`: the four API calls in `chrome-<major>-https.json` merged into one ranking (they never disagree). It covers the GET, the message POST, the join POST with `x-context-properties`, the PATCH with `x-audit-log-reason`, uploads (the multipart POST and the `PUT` to the upload URL) and every other API request.
- `IMAGE_ORDER`, for `Destination::Image`: the `<img>` in `chrome-<major>-image.json`, with `range` before `priority` from the `<video>` in `chrome-<major>-video.json` (an image never sends it).
- `UPGRADE_ORDER`, for `net::websocket_emulation()`: the upgrade in `chrome-<major>-wss-cross-site.json`, title case as Chrome writes it, without `Sec-WebSocket-Extensions` (never offered, below).

wreq applies them through an `OrigHeaderMap` just before the request is written, after it has added `content-length`, the cookie store's `cookie` fields, `Host` and the websocket headers. A name the list doesn't hold is never dropped: it follows the listed names, in the order kyoko inserted it (wreq's `OrigHeaderMap::call` and `call_visit`). Today that happens only to `host` on HTTP/1.1 (on HTTP/2 the host is `:authority`, so only mockcord's plain HTTP/1.1 sees it). A list name kyoko doesn't send, such as `x-debug-options`, is skipped. The lists add and remove nothing. `net::ws::connect` adds `Pragma: no-cache`, `Cache-Control: no-cache` and `Accept-Encoding: gzip, deflate, br, zstd` (`fingerprint::ACCEPT_ENCODING`, replacing wreq's `zstd,gzip,deflate,br`) to every upgrade and removes any `Sec-WebSocket-Extensions`.

Chrome's order for headers a page sets is fixed but neither alphabetical nor the page's order (it looks like a hash table's), so a header set the capture didn't cover could come out elsewhere in a real browser. Re-capture whenever kyoko adds a header.

Seam tests hold the wire to the fixtures, by name and order (`mockcord::hello::order_difference`; a name kyoko sends that Chrome's capture lacks fails it):

- `session::tls_fingerprint::api_requests_send_their_headers_in_chromes_order`: kyoko's HTTP/2 GET (history), message POST, join POST (`AcceptInvite`) and PATCH with a reason (`TimeoutMember`), against the hello listener, each against the fixture request of the same kind, with two seeded cookies. Every request of a matching kind on the connection is checked. Only `x-debug-options` is missing.
- `session::tls_fingerprint::uploads_send_their_headers_in_chromes_api_order`: the upload `PUT` (mockcord, HTTP/1.1, `host` left out) against `API_ORDER`.
- `session::tls_fingerprint::the_gateway_upgrade_runs_over_tls_with_the_web_client_headers` and `bridge::qr_login::a_refused_remote_auth_upgrade_over_tls_names_the_http_status`: names, case and order against `chrome-<major>-wss-cross-site.json`, `Host` first, the `Pragma`, `Cache-Control`, `Accept-Encoding`, `Upgrade` and `Sec-WebSocket-Version` values equal, `Connection: upgrade`, and only `Sec-WebSocket-Extensions` missing.
- `bridge::media_fetch::media_fetches_present_chromes_client_hello_and_image_request`: the image's order, no missing name, its values (`accept`, `accept-encoding`, `sec-fetch-*`, `priority`, the user agent and client hints), flags and HEADERS priority (weight 147) against `chrome-<major>-image.json`.
- `bridge::media_fetch::video_fetches_send_chromes_video_request`: a ranged video fetch (`MediaFetcher::open_range`) with the same checks against `chrome-<major>-video.json`, `range: bytes=0-` included.

**API POST** (a message send), before and after ticket 74 part 2a. A GET is the same without `content-length`, `content-type` and `origin`; the join POST adds `x-context-properties` after `x-discord-timezone`, the PATCH `x-audit-log-reason` after `x-super-properties`.

| # | Chrome 156 | kyoko before | kyoko now |
|---|---|---|---|
| 1 | `content-length` | `user-agent` | `content-length` |
| 2 | `sec-ch-ua-platform` | `accept` | `sec-ch-ua-platform` |
| 3 | `authorization` | `accept-encoding` | `authorization` |
| 4 | `x-installation-id` | `accept-language` | `x-installation-id` |
| 5 | `x-debug-options` | `x-discord-locale` | `sec-ch-ua` |
| 6 | `sec-ch-ua` | `x-discord-timezone` | `sec-ch-ua-mobile` |
| 7 | `sec-ch-ua-mobile` | `sec-ch-ua` | `x-discord-timezone` |
| 8 | `x-discord-timezone` | `sec-ch-ua-mobile` | `x-super-properties` |
| 9 | `x-super-properties` | `sec-ch-ua-platform` | `x-discord-locale` |
| 10 | `x-discord-locale` | `sec-fetch-dest` | `user-agent` |
| 11 | `user-agent` | `sec-fetch-mode` | `content-type` |
| 12 | `content-type` | `sec-fetch-site` | `accept` |
| 13 | `accept` | `priority` | `origin` |
| 14 | `origin` | `authorization` | `sec-fetch-site` |
| 15 | `sec-fetch-site` | `x-super-properties` | `sec-fetch-mode` |
| 16 | `sec-fetch-mode` | `x-installation-id` | `sec-fetch-dest` |
| 17 | `sec-fetch-dest` | `referer` | `referer` |
| 18 | `referer` | `origin` | `accept-encoding` |
| 19 | `accept-encoding` | `content-type` | `accept-language` |
| 20 | `accept-language` | `cookie`... | `cookie`... |
| 21 | `cookie`... | `content-length` | `priority` |
| 22 | `priority` | | |

Kyoko sends no `X-Debug-Options`. The capture includes it because ticket 74 lists it among the web client's headers; this capture can't confirm that the web client sends it.

**Image** (an `<img>` from another site). Kyoko now sends Chrome's list exactly, with the same values apart from `referer` (`https://discord.com/`).

| # | Chrome 156 | kyoko before | kyoko now |
|---|---|---|---|
| 1 | `sec-ch-ua-platform` | `user-agent` | `sec-ch-ua-platform` |
| 2 | `user-agent` | `accept` | `user-agent` |
| 3 | `sec-ch-ua` | `accept-encoding` | `sec-ch-ua` |
| 4 | `sec-ch-ua-mobile` | `accept-language` | `sec-ch-ua-mobile` |
| 5 | `accept` | `priority: i` | `accept` |
| 6 | `sec-fetch-site` | `referer` | `sec-fetch-site` |
| 7 | `sec-fetch-mode` | `sec-ch-ua` | `sec-fetch-mode` |
| 8 | `sec-fetch-dest` | `sec-ch-ua-mobile` | `sec-fetch-dest` |
| 9 | `sec-fetch-storage-access: active` | `sec-ch-ua-platform` | `sec-fetch-storage-access: active` |
| 10 | `referer` | `sec-fetch-dest` | `referer` |
| 11 | `accept-encoding` | `sec-fetch-mode` | `accept-encoding` |
| 12 | `accept-language` | `sec-fetch-site` | `accept-language` |
| 13 | `priority: i` | | `priority: i` |

HEADERS weight: Chrome 147, kyoko 220 before, 147 now.

**Websocket upgrade**, gateway and QR alike (both cross-site from discord.com, so no `Cookie` on either side):

| # | Chrome 156 | kyoko before | kyoko now |
|---|---|---|---|
| 1 | `Host` | `user-agent` | `Host` |
| 2 | `Connection: Upgrade` | `origin` | `Connection: upgrade` |
| 3 | `Pragma: no-cache` | `accept-language` | `Pragma: no-cache` |
| 4 | `Cache-Control: no-cache` | `sec-websocket-version: 13` | `Cache-Control: no-cache` |
| 5 | `User-Agent` | `upgrade: websocket` | `User-Agent` |
| 6 | `Upgrade: websocket` | `connection: upgrade` | `Upgrade: websocket` |
| 7 | `Origin` | `sec-websocket-key` | `Origin` |
| 8 | `Sec-WebSocket-Version: 13` | `accept-encoding: zstd,gzip,deflate,br` | `Sec-WebSocket-Version: 13` |
| 9 | `Accept-Encoding: gzip, deflate, br, zstd` | `host` | `Accept-Encoding: gzip, deflate, br, zstd` |
| 10 | `Accept-Language` | | `Accept-Language` |
| 11 | `Sec-WebSocket-Key` | | `Sec-WebSocket-Key` |
| 12 | `Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits` | | |

What wreq can and can't reproduce, read at the pinned revs:

- **HTTP/2 order: yes**, as above. Name case does not exist on HTTP/2. Not reproducible: priority chaining, and HPACK's indexing choices, which the capture does not compare.
- **HTTP/1.1 upgrade case and order: yes.** `OrigHeaderMap` writes listed names with the given spelling, in order (`call_visit`, used by wreq-proto's `proto/http1/role.rs`). `Host` is in the map by then (wreq's client layer adds it), so it goes first as `Host`. Kyoko setting `Accept-Encoding` replaces wreq's, because tower-http's decoder only fills in a missing one.
- **`Connection: Upgrade` value: no.** wreq's websocket builder inserts `connection: upgrade` after the caller's headers (`src/client/ws.rs`), overwriting any value kyoko sets. The name is spelt `Connection`; the value stays lowercase without a patch to wreq (the user's decision: no patch).
- **permessage-deflate: no, and kyoko must never offer it.** Neither wreq nor tungstenite 0.29 implements RFC 7692. If kyoko sent Chrome's `Sec-WebSocket-Extensions` line and a server accepted it, compressed frames (RSV1 set) would arrive and tungstenite would drop the connection. Discord does not appear to use it: its gateway docs list only zlib-stream and zstd-stream, and in [discord-api-docs discussion #3944](https://github.com/discord/discord-api-docs/discussions/3944) (opened 2021-10-10, saying the Discord client sends the offer) devsnek answered on 2021-10-12 "We don't have any current plans to support this extension" (read 2026-10-27). So the line stays missing, and every gateway or QR connect shows that gap to whoever terminates TLS.

### Recommendation

Keep claiming Chrome 156 (`CHROME_MAJOR = 156`) and use wreq-util's Chrome154 TLS and HTTP/2 options for it, with the emulation headers cleared. Chrome 156's handshake equals Chrome154's in every field the capture compares, so no `TlsOptions` override is needed for https and kyoko does not need to claim 154. Websockets need the ALPS override above. Re-capture at every Chrome bump (below).

### Bumping Chrome

Run this when a new Chrome major reaches full stable (Chromium Dash, see "Chrome major and the brand list"). Nothing here contacts Discord; the capture stays on localhost.

1. Update the local Chrome to the new major (`chrome://settings/help`, or the installer) and note its full version.
2. Record it: `cargo xtask tls-capture --client chrome --h2 --record chrome-<major>`. Check the printed connections: the page, `/fetch` and the API calls on `h2` to `localhost`, `/image.gif` on its own `h2` connection, then `/ws` and the cross-site upgrade on `http/1.1`. It writes `chrome-<major>-https.json`, `-wss.json`, `-image.json` and `-wss-cross-site.json` in `tools/mockcord/fixtures/tls/`. Keep the older major's files until the bump is merged.
3. Compare with the old major: `cargo xtask tls-capture --client chrome --h2 --compare chrome-<old major>` prints handshake differences, and a diff of the JSON files shows header changes. Look at every header value kyoko copies, as `image/jxl` was for 156: `sec-ch-ua` (against `fingerprint::sec_ch_ua()` for the new major), `accept` on `/fetch` and on the image (`IMAGE_ACCEPT`), `accept-encoding`, `accept-language`, `priority`, any new `sec-fetch-*` header, the API header order, and the upgrade's names and order. Fix copied values in `crates/protocol/src/fingerprint.rs` (or `rest`, `gateway`, `login::qr`) together with their tests.
4. Compare wreq's profile with the new capture: `cargo xtask tls-capture --client wreq --h2 --compare chrome-<major>`.
5. With no https and no wss difference, add the major to `net::profile` in `crates/net/src/chrome.rs` beside the profile it equals.
6. With a difference, find a newer wreq-util profile that matches: bump the `wreq-util` rev in `[workspace.dependencies]` and the `wreq`, `btls` and `btls-sys` `[patch.crates-io]` revs to the ones that wreq-util uses, update `Cargo.lock`, run `cargo deny check` (licence, the zstd ignores) and `cargo tree -e features -i btls-sys`, rebuild the shared BoringSSL prebuilt for a new `btls` rev ("Sharing one BoringSSL between worktrees"), and repeat step 4. If no profile matches, set the differing `TlsOptions` or `Http2Options` fields in `crates/net/src/chrome.rs` instead. Update "Pins".
7. Bump `CHROME_MAJOR` in `crates/protocol/src/fingerprint.rs`. The build fails until step 5 or 6 maps it. Set `CHROME_156_BRANDS` in `crates/session/tests/session/fingerprint.rs` (renamed for the new major) to the captured `sec-ch-ua`.
8. Run the TLS seam tests: `cargo test -p kyoko-session --test session tls_fingerprint` and `cargo test -p kyoko-bridge --test bridge -- media_fetch qr_login`. They read `chrome-<CHROME_MAJOR>-*.json` and fail until the fixtures exist and match: ClientHello, SETTINGS, WINDOW_UPDATE, priority, pseudo-header order, and the header orders and values of step 9.
9. Rebuild the header order lists in `crates/net/src/order.rs` from the new fixtures ("Header order"): `API_ORDER` from the four `/api/` requests in `-https.json` (merge them into one ranking; if two disagree, record that in this file and ask), `IMAGE_ORDER` from `-image.json`, `UPGRADE_ORDER` from `-wss-cross-site.json` without `Sec-WebSocket-Extensions`. Check the copied values the order tests also compare: the image's `priority` and HEADERS weight (`IMAGE_WEIGHT` in `crates/net/src/chrome.rs`), `sec-fetch-storage-access`, and the upgrade's `Pragma`, `Cache-Control` and `Accept-Encoding`. Then run the order seam tests: `cargo test -p kyoko-session --test session tls_fingerprint` and `cargo test -p kyoko-bridge --test bridge -- media_fetch qr_login`. A new name in Chrome's list that kyoko doesn't send, or one kyoko sends that Chrome's list lacks, fails them: decide it (add the header, or record the gap) rather than editing the test.
10. Delete the old major's Chrome fixtures, re-record `wreq-chrome<profile>-*.json` if the profile changed (`--client wreq --h2 --record wreq-chrome<profile>`), and update this file ("What Chrome sends", "Header order", "Chrome major and the brand list", versions and dates), `docs/media.md` for image headers, and `docs/testing.md` if a command changed.

### Build cost

Measured on the development machine (i9-12900HK, `CARGO_BUILD_JOBS=3`, sccache as `RUSTC_WRAPPER`, other worktrees building at the same time), 2026-10-27:

- **First build of `btls-sys`**, the build script that runs CMake, MSBuild and bindgen: 383 s for dev (RelWithDebInfo, because dependencies build at `opt-level = 3` with debug info) and 201 s for release. The wreq, btls and http2 Rust crates add about 15 s. A changed `BORING_BSSL_*`, `CMAKE*` or `HOST_CMAKE_TOOLCHAIN_FILE` value reruns it (about 4 min with the CMake build dir already there).
- **No-op rebuild** of `kyoko-net`: 0.9 s.
- **sccache** caches none of the C build: its C/C++ miss count did not move during it. CMake's Visual Studio generator ignores the compiler launcher that `btls-sys` passes.
- **Git fetch**: `~/.cargo/git` grew by 672 MB, mostly the BoringSSL submodule (192 MB database, 455 MB `btls` checkout with it).
- **Target dir**: about 0.6 GB per profile (dev: 337 MB `btls-sys` build dirs, a 252 MB patched copy of BoringSSL among them, 186 MB of new crates, 69 MB for the probe example; release: 301 MB of `btls-sys` build dirs).
- **Release `kyoko.exe`**: 78,949,888 bytes today, 83,832,832 bytes (+4.66 MiB, +6.2 %) with a temporary call into `crates/net` (client build, a GET and a websocket open), while reqwest, rustls and aws-lc are still linked. With REST and the gateway on wreq (tickets 70 and 71) it was 83,962,880 bytes. With the QR socket, media and theme links on wreq too and reqwest, rustls and aws-lc gone (tickets 72 and 73) it is 78,870,528 bytes (-4.86 MiB, -6.1 %), 79 KB smaller than before wreq came in.

Native Windows builds of `btls-sys` assemble BoringSSL's assembly unless they cross-compile, which needs NASM. `.cargo/config.toml` points `HOST_CMAKE_TOOLCHAIN_FILE` at `.cargo/boringssl-no-asm.cmake`, which sets `OPENSSL_NO_ASM` (and the release C runtime, which `btls-sys` stops setting when a toolchain file is given). The cost is slower bulk crypto; the TLS 1.3 cipher order is unchanged, because the profile sets `aes_hw_override`, as the capture shows. libclang was found in `C:\Program Files\LLVM` without `LIBCLANG_PATH`.

**Sharing one BoringSSL between worktrees.** `BORING_BSSL_INSTALL_DIR` does not work on Windows at this rev: it copies `libcrypto.a` and `libssl.a`, and MSVC builds `crypto.lib` and `ssl.lib`, so the build script fails. Copying by hand works: after a release build, copy `target/release/build/btls-sys-*/out/build/Release/{crypto,ssl}.lib` into `<dir>/lib` and `target/release/build/btls-sys-*/out/boringssl/include` into `<dir>/include`, outside the repo, one dir per `btls` rev. Then set `BORING_BSSL_PATH=<dir>` and `BORING_BSSL_ASSUME_PATCHED=1`: the build script runs bindgen only (2.6 s), CMake is not needed, and the capture still shows no difference from Chrome 156. Release libraries link into dev builds too (both use the `/MD` runtime).

**CI.** No workflow file changed. `windows-latest` has CMake on `PATH`, LLVM in `C:\Program Files\LLVM` and Git, and the toolchain file removes the need for NASM, so `ci.yml` should build and test the workspace as it is, with the first run paying the BoringSSL fetch and build (`Swatinem/rust-cache` keeps `~/.cargo/git` and `target` afterwards). `release.yml` builds `-p kyoko`, which links `crates/net` since ticket 70, so its Windows job pays the BoringSSL build too. Its ubuntu `publish` job builds `xtask`, which does not depend on `crates/net`, so it builds no BoringSSL, but cargo still fetches the git pins and the BoringSSL submodule to load the workspace. None of this has run on CI yet.

## Voice

**Decided (the user, 2026-10-28): UDP, option A.** See "Decision" below. `crates/voice` now holds the connection core (ticket 97, `docs/voice.md`), which the session drives since ticket 98; the build trial lives on as its `voice_trial` example. The voice upgrade presents the gateway's Chrome wss ClientHello and `UPGRADE_ORDER`, with no Cookie. The plan is `nguyenvu/.research/voice-plan/plan.md` (sections 1–5 and 7); facts it rests on were rechecked here.

Op 4 (ticket 98) carries the docs' fields in their order: `guild_id`, `channel_id`, `self_mute`, `self_deaf`, `self_video`, `preferred_region`, `preferred_regions`, `flags` ([gateway events](https://docs.discord.food/gateway/gateway-events#update-voice-state), read 2026-10-28). The regions are READY's `geo_ordered_rtc_regions`; the web client ranks regions by pinging `latency.discord.media`, which kyoko does not do, so its order is Discord's geographic one. `flags` is 2 (`ALLOW_VOICE_RECORDING`) without `CLIPS_ENABLED` (1), which belongs to the desktop app's clips; the web client's real value is unconfirmed until a capture.

### What the web client does

From [voice connections](https://docs.discord.food/topics/voice-connections) (userdoccers `2e49806`, 2026-06-24, read 2026-10-28):

- A browser cannot open a raw UDP socket, so the web client uses `protocol: "webrtc"`: ICE, DTLS 1.2+ and SRTP to Discord's SFU, with a stripped SDP fragment from Chrome's own offer in Select Protocol. It uses no IP discovery, no `mode`, no `secret_key` and none of the `aead_*` transport modes ("WebRTC Connections"). DAVE frames ride in Chrome's encoded transforms (`encodedInsertableStreams: daveEnabled`).
- Discord's answer has `a=setup:passive`, so the browser is the DTLS client and sends the ClientHello the SFU sees on every call.
- `protocol: "udp"` with IP discovery, `aead_aes256_gcm_rtpsize` or `aead_xchacha20_poly1305_rtpsize` is what the desktop app and bots use, and what oxidcord implements. Its payload types (Opus 120, H264 101/102) are not the browser's ("Do not rewrite browser WebRTC payload types to UDP defaults", Opus "usually payload type 111").
- The web client heartbeats every `min(heartbeat_interval, 5000)` ms.

### Chrome 156 on localhost

`cargo xtask webrtc-capture --client chrome` (`docs/testing.md`, "WebRTC offer and DTLS hello"): Chrome 156.0.8078.48 on a throwaway profile, a page on `127.0.0.1` with `RTCPeerConnection({bundlePolicy: "max-bundle"})` and `recvonly` audio and video transceivers, answered by a local str0m peer. Fixtures: `tools/mockcord/fixtures/webrtc/chrome-156-offer.sdp`, `-offer.json`, `-dtls.json`. Two further runs compared with 0 differences. ICE, DTLS and the connection came up every time.

The offer:

- Session: `a=group:BUNDLE 0 1`, `a=extmap-allow-mixed`, `a=msid-semantic: WMS`. Each section: `UDP/TLS/RTP/SAVPF`, `a=ice-options:trickle`, `a=fingerprint:sha-256`, `a=setup:actpass`, `a=rtcp-mux`, `a=rtcp-rsize`, `a=rtcp-xr:rcvr-rtt=all`.
- Audio (`mid 0`): `111 opus/48000/2` (`minptime=10;useinbandfec=1`, `transport-cc`), `63 red`, `9 G722`, `0 PCMU`, `8 PCMA`, `13 CN`, `110` and `126 telephone-event`. Extmaps 1 `ssrc-audio-level`, 2 `abs-send-time`, 3 `transport-wide-cc`, 4 `sdes:mid`.
- Video (`mid 1`), 42 payload types: VP8 96 (RTX 97); VP9 profiles 0, 2, 1, 3 on 98, 100, 35, 37; H264 on 102, 104, 108, 114, 116, 39, 41, 43, 118, 120 (profiles `42001f`, `42e01f`, `4d001f`, `f4001f`, `64001f`, packetization modes 1 and 0); AV1 profiles 0 and 1 on 45, 47; H265 profiles 1 and 2 on 49, 51 (Chrome offers H265 only where the GPU decodes it, as here); each with RTX; then `122 red`, `124 ulpfec`, `53 flexfec-03`. Every video codec has `goog-remb`, `transport-cc`, `ccm fir`, `nack`, `nack pli`. Extmaps 14 `toffset`, 2 `abs-send-time`, 13 `video-orientation`, 3 `transport-wide-cc`, 5 `playout-delay`, 6 `video-content-type`, 7 `video-timing`, 8 `color-space`, 4 `sdes:mid`, 10 `rtp-stream-id`, 11 `repaired-rtp-stream-id`. These match the extmap ids in the docs' fragment example.

The DTLS ClientHello is BoringSSL's, 1,413 bytes, so it is split over two datagrams (1,463 bytes with headers):

- JA4 `dd3i140900_fa23b50283d1_01b4b87481e4`; record version DTLS 1.0, `legacy_version` DTLS 1.2, `supported_versions` DTLS 1.3 and 1.2, empty cookie, no GREASE.
- 14 ciphers: the three TLS 1.3 suites, ECDHE-ECDSA and ECDHE-RSA with AES-128-GCM and ChaCha20, the four ECDHE CBC-SHA suites, and RSA AES-128-GCM, AES-128-CBC, AES-256-CBC.
- Groups X25519MLKEM768, X25519, P-256, P-384, with key shares for X25519MLKEM768 (1,216 bytes) and X25519. Nine signature algorithms (ECDSA P-256/P-384, RSA-PSS and PKCS#1 with SHA-256/384/512, PKCS#1 SHA-1). `psk_key_exchange_modes` 1, `renegotiation_info`, `extended_master_secret`, `ec_point_formats`, `use_srtp` with `SRTP_AES128_CM_SHA1_80`, `SRTP_AEAD_AES_256_GCM`, `SRTP_AEAD_AES_128_GCM` in that order.
- Extension order is permuted per connection, as in Chrome's TLS.

### str0m against it

str0m 0.24.1 (`str0m-wincrypto` 0.8.1, Windows CNG under the pure-Rust `dimpl` DTLS) recorded with `--client str0m` (fixtures `str0m-wincrypto-dtls12-*` and `str0m-wincrypto-auto-*`). `str0m-rust-crypto` 0.6 also runs `dimpl` and sends the same shape (it was tried during this ticket: the same differences, with a ChaCha20 suite added). The capture keeps only the wincrypto backend:

| | Chrome 156 | str0m, DTLS 1.2 (default) | str0m, `auto` |
|---|---|---|---|
| JA4 | `dd3i140900_fa23b50283d1_01b4b87481e4` | `dd2i020500_177d36ae841b_cd314463c40d` | `dd3i040900_5794a053403b_9ab7eb0b348d` |
| Versions | DTLS 1.3, 1.2 | 1.2 only | 1.3, 1.2 |
| Ciphers | 14 | 2 (ECDHE-ECDSA AES-GCM) | 4 |
| Groups, key shares | ML-KEM hybrid first, two shares | no ML-KEM, no shares | no ML-KEM, X25519 only |
| Signature algorithms | 9 | 2 | 2 |
| Record version | DTLS 1.0 | DTLS 1.2 | DTLS 1.2 |
| Other | PSK modes, renegotiation_info, SRTP order 0001,0008,0007 | no PSK modes or renegotiation_info, SRTP order 0008,0007,0001 | adds `padding`, no PSK modes |

`--compare chrome-156` finds 11 differences (1.2), 12 (1.3) and 10 (auto) in the hello. str0m's own offer differs in 48 fields (its codec list, payload types and extmaps); that part does not matter for B, because kyoko would build its Select Protocol fragment and codec list from the recorded Chrome offer, not from str0m's. The hello is what cannot be copied as text.

BoringSSL itself does DTLS: the pinned `btls` (kyoko's TLS library since ticket 70) has `SslMethod::dtls()` and `set_tlsext_use_srtp`. A str0m `DtlsProvider` on `btls`, configured as Chrome's WebRTC configures BoringSSL, is the only route found to Chrome's hello; it is not written.

### Build trial

`crates/voice` (`cargo run -p kyoko-voice --example voice_trial --features openh264`, `crates/voice/examples/voice_trial/`) builds oxidcord's voice dependencies and runs each once: libopus 1.6.1 encode and decode (`opus` 0.4 on `opusic-sys` 0.7.6, bundled), `rubato` 5.0.1 resampling, `nnnoiseless` 0.5.2 (since v1 115 the trial runs `kyoko-denoise` instead), cpal's WASAPI devices, both AEAD transport modes, a two-member DAVE MLS group on `openmls` 0.9.2 (P-256 suite, welcome, agreed media key; `openmls_rust_crypto` 0.6 at the time, kyoko's BoringSSL provider since ticket 99), Media Foundation H.264 transforms (3 hardware and 1 software encoder, 1 decoder here), `GraphicsCaptureSession::IsSupported`, and with `--features openh264` an `openh264` 0.9.9 encode and decode. All pass on this machine.

- **libopus under kyoko's toolchain file.** It builds with `HOST_CMAKE_TOOLCHAIN_FILE=.cargo/boringssl-no-asm.cmake` unchanged. `CMakeCache.txt` shows the toolchain file, `-MD`, and `OPUS_X86_MAY_HAVE_SSE`, `SSE2`, `SSE4_1` and `AVX2` all `ON` (runtime dispatch; `PRESUME_SSE`/`SSE2` `ON`). `opus.lib` (3.4 MB) holds `silk_NSQ_del_dec_sse4_1`, `celt_pitch_xcorr_avx2` and `opus_select_arch`, and references `MSVCRT` only, no `LIBCMT`. No fix needed. `OPENSSL_NO_ASM` is irrelevant to libopus.
- **openh264.** Its build script finds no NASM and falls back to plain C ("Failed to compile NASM files, not using any assembly"), so a software encode would be slower than Cisco's binary.
- **Windows features.** `Foundation`, `Graphics_Capture`, `Win32_Media_MediaFoundation`, `Win32_System_Com` on the locked `windows` 0.62.2. MF and COM: Media Foundation must tolerate COM already initialised in another apartment (cpal initialises it on the same thread); the trial treats `RPC_E_CHANGED_MODE` as "already initialised", as oxidcord does.

Measured on the development machine, 2026-10-28, `CARGO_BUILD_JOBS=3`, other worktrees building at the same time (so ±30 %), building `-p kyoko-voice --features openh264 --examples` into an empty target dir:

| | dev, sccache off | release, sccache off | dev, sccache on, cold | dev, sccache on, warm |
|---|---|---|---|---|
| Whole trial (all deps) | 378 s | 199 s | 248 s | 96 s |
| `opusic-sys` build script (CMake, MSBuild) | 54.5 s | 54.0 s | 56.9 s | 47.9 s |
| `openh264-sys2` build script (`cc`) | 64.4 s | 60.5 s | 102.1 s | 29.6 s |
| `openmls` | 72.8 s | 16.9 s | 19.6 s | 18.5 s |
| other large crates | `nnnoiseless` 97 s, `windows` 89 s, `kyoko-voice` 70 s, `libcrux-sha3` 24 s | `windows` 51 s, `nnnoiseless` 21 s | | |
| No-op rebuild | 0.93 s | 0.91 s | | |

- **sccache.** libopus is never cached: CMake's Visual Studio generator ignores the compiler launcher, as with `btls-sys`. openh264's 86 C++ files are: `cc` sees `RUSTC_WRAPPER` and compiles through sccache, so the warm build hit all 86 (the remaining 30 s is NASM probing and archiving). Rust crates hit as usual (184 hits on the warm build; sccache's counters are shared with the other lanes, so the counts are approximate).
- **Disk.** A standalone target dir for the trial takes 0.95 GB dev and 0.66 GB release; the libopus build dir is 14 MB dev and 7 MB release, openh264's 23 MB and 5 MB. The `opusic-sys` source is 30 MB in `~/.cargo/registry`.
- **Release `kyoko.exe`.** 79,469,568 bytes on `a976864`. With a temporary call into the trial (reverted, never committed): 84,065,792 bytes without openh264 (+4.38 MiB, +5.8 %) and 84,789,248 with it (+5.07 MiB, +6.7 %; openh264 alone +0.69 MiB). The trial calls each library once, so the real port will add oxidcord's own voice and video code on top (the plan estimates 1–2 MB).
- **Licences and advisories.** `cargo deny check` passes with one new ignore: RUSTSEC-2026-0387 (non-constant-time arithmetic in `crypto-bigint` 0.5.5, fixed only in 0.7.6). It comes from `p256`/`p384` 0.13 under `openmls_rust_crypto`, `openmls_basic_credential` and `hpke-rs-rust-crypto` (the DAVE signing and HPKE keys), and from `dimpl` under str0m in xtask; no 0.5 or 0.13 release fixes it. Before DAVE ships (V3), either accept it for local timing only or give openmls a crypto provider that does not use RustCrypto's 0.13 curves (BoringSSL through `btls` is linked already). hpke-rs is MPL-2.0 and libopus BSD-3; `THIRD_PARTY_NOTICES.md` gets them when voice ships.
- `cargo tree -p kyoko -e normal` still shows no rustls, aws-lc-sys, ring or TLS-enabled tokio-tungstenite (only wreq's own `tokio-tungstenite` 0.29 with default features off), and `kyoko-voice` brings none of them either.

### The choice

- **A. UDP, ported from oxidcord.** Weeks sooner. The voice server sees a session that identified as Chrome on the web switch to the desktop app's transport: `protocol: "udp"`, IP discovery, an `aead_*` mode, Opus on 120, no DTLS at all. That is a protocol-level difference, not a field-level one, and it shows on every call. Whether Discord acts on it is unknown; user-account clients (endcord, discordo) use UDP today.
- **B. WebRTC as Chrome.** ICE, DTLS, SRTP, RTCP (RR/SR, NACK, PLI, transport-cc) and the answer synthesis. The SDP side is cheap: the fragment, codec list and payload types can be built from the recorded offer above. The DTLS side is not: str0m's hello differs from Chrome's in 10–12 fields, and the only matching hello found is BoringSSL's, which means writing a str0m DTLS provider on `btls` (Chrome's options: DTLS 1.3 and 1.2, the 14 suites, X25519MLKEM768 first, the SRTP order, extension permutation). Several weeks more than A, and the result still needs a capture of the real SFU's behaviour, which only a user run can give.
- **C. Everything above the transport once, behind a `Transport` seam.** Signalling on `net::ws` (v9, web heartbeat, `rtc_connection_id`, two layers), DAVE, Opus, jitter, mixer, video codecs, session and UI are shared by A and B. Ship A first with the gap recorded here; B is a follow-up.

**Recommendation: C**, and if B follows, do its DTLS on `btls`, not on str0m's default `dimpl` (the measurement shows dimpl cannot be made to look like Chrome without rewriting its hello). If "nothing more" must hold for voice from the first release, choose B and expect voice to land weeks later.

**openh264: leave it out.** Media Foundation is licensed with Windows and does H.264 encode and decode here in hardware and software; openh264 costs 0.69 MiB, a 60–100 s C++ build (no NASM, so its slow C path), and Cisco's patent cover only applies to Cisco's downloaded binary, not a source build. Keep it behind the off-by-default `openh264` feature for testing only. Windows N editions without the Media Feature Pack show "video unavailable".

### Decision

The user chose on 2026-10-28: **A, UDP.** In their words: "if others use udp then i dont know why we would use webrtc".

- Voice uses `protocol: "udp"` with IP discovery, `aead_aes256_gcm_rtpsize` (or `aead_xchacha20_poly1305_rtpsize` when that is all the server offers) and DAVE, as oxidcord, the desktop app, discordo and endcord do. Signalling still goes over `net::ws` with the web client's identify, heartbeat and simulcast layers.
- This is a known gap, like permessage-deflate: a session that identifies as Chrome on the web uses the desktop app's voice transport, with no DTLS and UDP payload types.
- The code keeps a thin `Transport` seam between the media pipeline and the UDP socket, because it costs little. No WebRTC work is planned.
- `cargo xtask webrtc-capture` and its fixtures in `tools/mockcord/fixtures/webrtc/` stay as the record of what Chrome 156 does, in case this is ever revisited.
- **openh264 stays out.** H.264 encode and decode use Media Foundation; the trial's `openh264` feature stays off and is never shipped.

### DAVE crypto provider

Decided by the user on 2026-10-28 (ticket 99): do it the way Discord does. Discord's libdave runs MLS on mlspp over OpenSSL or BoringSSL. `openmls_rust_crypto`, which oxidcord, davey, songbird and discord.js use, pulls `p256`/`p384` 0.13 and so `crypto-bigint` 0.5 (RUSTSEC-2026-0387, non-constant-time arithmetic, no fixed 0.5 release). So `crates/dave` keeps oxidcord's DAVE protocol code but gives openmls its own provider, `dave::Provider` (`BoringCrypto` for crypto and randomness, `openmls_memory_storage` for state), on the BoringSSL that `btls` already links for TLS. No `openmls_rust_crypto`, `openmls_basic_credential` or `hpke-rs` is linked; the MLS signer is `dave::SigningKey`.

It supports one ciphersuite, `MLS_128_DHKEMP256_AES128GCM_SHA256_P256` (DAVE v1); any other returns `UnsupportedCiphersuite`, and other hash, AEAD, KDF, KEM or signature choices return the matching `Unsupported*` error.

| Need | BoringSSL through |
|---|---|
| P-256 ECDSA keys, sign and verify (SHA-256, DER signatures, uncompressed SEC1 public keys, 32-byte scalars) | `btls::ec`, `btls::ecdsa`, `btls::bn` |
| HPKE base mode, DHKEM(P-256, HKDF-SHA256), HKDF-SHA256, AES-128-GCM: seal, open, sender and receiver export, DeriveKeyPair | `btls-sys`: `EVP_HPKE_CTX_*`, `EVP_HPKE_KEY_*`, `EVP_hpke_p256_hkdf_sha256` |
| HKDF-SHA256 extract and expand | `btls-sys`: `HKDF_extract`, `HKDF_expand` |
| HMAC-SHA256, SHA-256 | `btls::hmac::Hmac`, `btls::hash` |
| AES-128-GCM for MLS (16-byte tag) and DAVE frames (8-byte truncated tag) | `btls::aead::AeadCtx` |
| Randomness | `btls::rand::rand_bytes` |

What btls covers at the pinned rev `1298875`: AEAD contexts with any tag length, HMAC, digests, EC keys and points, ECDSA with DER, and `RAND_bytes`. What it lacks: a usable HPKE (its `hpke::HpkeKey::dhkem_p256_sha256` actually initialises the X25519 KEM, and there is no seal, open or export) and HKDF. BoringSSL itself has both, including the P-256 HPKE KEM, and btls-sys's generated bindings export them, so `crates/dave/src/hpke.rs` and the provider call them directly. Nothing had to be built from primitives. The HPKE code is checked against RFC 9180's test vectors for this suite (A.3.1, base mode: DeriveKeyPair, `enc` from the fixed ephemeral seed, two sealed messages, open, three exports), copied from BoringSSL's `hpke_test_vectors.txt`.

Interop: `crates/dave/tests/interop.rs` puts a kyoko member and a `davey` 0.1.5 member (openmls 0.8 on RustCrypto, a dev-dependency only) in one group, each way round. davey verifies kyoko's BoringSSL signatures and opens its HPKE, kyoko opens davey's welcome, and each decrypts the other's Opus frames.

`cargo tree -p kyoko -e normal -i crypto-bigint` finds nothing; neither do `p256`, `hpke-rs` or `openmls_rust_crypto`. The only `crypto-bigint` left in the workspace is davey's, in the interop test; xtask's str0m (wincrypto backend) no longer brings it. davey 0.1.5's own tree (openmls 0.8 with `openmls_rust_crypto` 0.5, `hpke-rs` 0.6 and libcrux 0.0.x) carries RUSTSEC-2026-0387, -0207, -0208 and -0212, so `deny.toml` excludes `davey` from cargo-deny's graph (`[graph] exclude`): it is a dev-dependency of one test and never ships. The RUSTSEC-2026-0387 ignore is gone.

## IDENTIFY

Keys in this order: `token`, `properties`, `compress` (`false`), `presence`, `capabilities`, `client_state`. There is no `large_threshold`; the user default is already 250.

`presence` is `{"activities":[],"status":"unknown","since":0,"afk":false}`, as the docs say a user client sends on a fresh IDENTIFY. Discord then applies the status from the user's settings, and READY's `user_settings_proto` restores it in `state` (`State::own_status`). After a fresh READY (not a resume) the session sends op 3 again only when Discord's copy would be wrong: while away (`Command::SetAfk(true)`, sent as `idle` with `afk: true`) or while a status write is still waiting or in flight, whose status it also keeps over READY's older one (`Core::restore_presence`). Otherwise kyoko sends no op 3 after IDENTIFY.

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
