# Client fingerprint

Kyoko presents itself as the Discord web client in Chrome, on both the gateway and REST. Everything that makes up that identity lives in `crates/protocol/src/fingerprint.rs`, plus the IDENTIFY builder in `crates/gateway/src/frames.rs`. When Chrome or Discord moves, update those two files.

## What is sent

- **API v9** on REST (`/api/v9`) and the gateway (`v=9`).
- **User agent**: Chrome's reduced form, `Chrome/<CHROME_MAJOR>.0.0.0`. The same string goes in the gateway handshake, every REST request, the build-number scrape and the QR login socket.
- **Super-properties**: one `IdentifyProperties` object, in the web client's key order: `os`, `browser`, `device`, `system_locale`, `browser_user_agent`, `browser_version`, `os_version`, `referrer`, `referring_domain`, `referrer_current`, `referring_domain_current`, `release_channel`, `client_build_number`, `client_event_source`, `has_client_mods`, `client_launch_id`, `launch_signature`, `client_app_state`, `client_heartbeat_session_id`. The gateway sends it as IDENTIFY `properties`, with `installation_id` appended once kyoko knows one. REST sends the same JSON without `installation_id`, base64-encoded, as `X-Super-Properties`. Apart from `installation_id`, the two are identical at any moment.
- **Launch ids**: `client_launch_id` is a fresh UUID per launch. `launch_signature` is a fresh UUIDv4 per launch with the twelve client-mod detection bits (`LAUNCH_SIGNATURE_MOD_BITS`) cleared, so it says "no client mods" as `has_client_mods: false` does.
- **Heartbeat session id**: `client_heartbeat_session_id` starts as a fresh UUID and is replaced by a new one once it is 30 minutes old (`HEARTBEAT_SESSION_LIFETIME`). `rest::ClientIdentity` checks its age every time it hands out properties, for a REST request or an IDENTIFY, so gateway and REST always carry the same id. The age comes from a `fingerprint::Clock`, the system clock unless a test injects one.
- **Build number**: at startup the session fetches `https://discord.com/app`, then reads the build number from its scripts, sentry first. Discovery takes at most 5 s and IDENTIFY waits for it. On failure the last discovered build is used (`settings.client.build_number`), then `FALLBACK_BUILD_NUMBER`. This scrape is the only request outside the API. It goes only to discord.com or loopback.
- **REST headers**: `X-Discord-Locale`, `X-Discord-Timezone` (the system IANA zone), `Accept-Language`, Chrome client hints, `Sec-Fetch-*`, `Priority`, `Origin` on non-GET requests, and a `Referer` that follows the open channel (`/channels/<guild>/<channel>`, `/channels/@me/<channel>`, or `/channels/@me` before a channel is opened). The QR login's ticket exchange, made from the login page, sends `Referer: https://discord.com/login` (`LOGIN_REFERER`).
- **Client hints**: `sec-ch-ua` is computed from `CHROME_MAJOR` the way Chrome computes it (below). For 156 it is `"Not:A-Brand";v="8", "Chromium";v="156", "Google Chrome";v="156"`.
- **Installation id**: taken from READY's `apex_experiments.installation` the first time it appears. It is stored and sent as `X-Installation-ID` and as IDENTIFY `properties.installation_id` from then on.
- **Context properties**: joining a server and sending a friend request from the Add Friend tab send `X-Context-Properties` (`{"location":"Add Friend"}` for the latter, `ContextProperties::relationship`). For a join it is base64 of the compact JSON `{"location","location_guild_id","location_channel_id","location_channel_type"}` (plus `location_message_id` for an invite in a message), in that order. The add-server dialog says `"Join Guild"` with the invite's server, channel and channel type; an invite shown in a message (`InviteSource::Message`, for invite cards in the timeline, not drawn yet) says `"Invite Button Embed"` with the message's channel and id (`fingerprint::ContextProperties`, `JoinLocation`, `rest::Request::context_properties`). No other request carries it yet (`docs/rest.md`, "Relationships"). The shape comes from the spike's notes and community research of the web client; userdoccers does not document the header, so check it on the next real capture.
- **Cookies**: one jar, filled from Discord's responses, plus `__dcfduid`, `__sdcfduid` and `_cfuvid` carried over from the web login page (`docs/sign-in.md`, "Web login").
- **Web login**: the WebView2 page gets the same user agent string. Its `Sec-CH-UA` stays WebView2's own, which kyoko cannot set.
- **Images**: CDN and media-proxy fetches send Chrome's `<img>` headers from `Fingerprint::image_headers`: the same user agent and client hints, an image `Accept`, `Referer: https://discord.com/`, `Sec-Fetch-Dest: image`, `Sec-Fetch-Mode: no-cors`, `Sec-Fetch-Site: cross-site` and `Priority: i`. They carry no cookies, because discord.com cookies are never sent to Discord's CDN hosts. See `docs/media.md`.

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

## Sharing and persistence

`rest::ClientIdentity` holds the fingerprint, the cookie jar, the installation id and the clock that ages the heartbeat session. Build one per launch with `session::client_identity(&store)` (`client_identity_with_clock` in tests), then pass it to login (`TokenLogin::validate`, `QrConfig::identity`, `session::login`) and to `SessionConfig::identity`. That way every request shares one jar and one id. The gateway reads IDENTIFY `properties` from the same identity through a `gateway::PropertiesSource` on every connect, so a re-identify carries the current build, heartbeat session id and installation id. The installation id and the jar are stored in the encrypted `kv` table. The session saves the jar whenever it changes and again on shutdown.

`SessionConfig::build_source` is the page to scrape: `session::discord_build_source()` against Discord, `Server::app_url()` against mockcord, or `None` to skip discovery.
