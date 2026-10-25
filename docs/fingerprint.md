# Client fingerprint

Kyoko presents itself as the Discord web client in Chrome, on both the gateway and REST. Everything that makes up that identity lives in `crates/protocol/src/fingerprint.rs`. When Chrome or Discord moves, update that file.

## What is sent

- **API v9** on REST (`/api/v9`) and the gateway (`v=9`).
- **User agent**: Chrome's reduced form, `Chrome/<CHROME_MAJOR>.0.0.0`. The same string goes in the gateway handshake, every REST request, the build-number scrape and the QR login socket.
- **Super-properties**: one `IdentifyProperties` object. The gateway sends it as IDENTIFY `properties`. REST sends the same JSON, base64-encoded, as `X-Super-Properties`. `client_launch_id` and `client_heartbeat_session_id` are fresh UUIDs per launch.
- **Build number**: at startup the session fetches `https://discord.com/app`, then reads the build number from its scripts, sentry first. Discovery takes at most 5 s and IDENTIFY waits for it. On failure the last discovered build is used (`settings.client.build_number`), then `FALLBACK_BUILD_NUMBER`. This scrape is the only request outside the API. It goes only to discord.com or loopback.
- **REST headers**: `X-Discord-Locale`, `X-Discord-Timezone` (the system IANA zone), `Accept-Language`, Chrome client hints, `Sec-Fetch-*`, `Priority`, `Origin` on non-GET requests, and a `Referer` that follows the open channel (`/channels/<guild>/<channel>`, `/channels/@me/<channel>`, or `/channels/@me` before a channel is opened).
- **Installation id**: taken from READY's `apex_experiments.installation` the first time it appears. It is stored and sent as `X-Installation-ID` from then on.
- **Cookies**: one jar, filled only from Discord's responses.

## Sharing and persistence

`rest::ClientIdentity` holds the fingerprint, the cookie jar and the installation id. Build one per launch with `session::client_identity(&store)`, then pass it to login (`TokenLogin::validate`, `QrConfig::identity`, `session::login`) and to `SessionConfig::identity`. That way every request shares one jar and one id. The installation id and the jar are stored in the encrypted `kv` table. The session saves the jar whenever it changes and again on shutdown.

`SessionConfig::build_source` is the page to scrape: `session::discord_build_source()` against Discord, `Server::app_url()` against mockcord, or `None` to skip discovery.
