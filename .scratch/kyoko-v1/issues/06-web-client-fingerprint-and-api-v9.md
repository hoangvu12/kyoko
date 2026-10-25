# 06 — Web-client fingerprint and API v9

**What to build:** kyoko presents one consistent Discord web-client identity on both the gateway and REST, and uses API v9 like the web client.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** resolved

- [x] REST and the gateway use API v9
- [x] A reduced Chrome user agent (<major>.0.0.0) and complete super-properties are used, consistent between gateway and REST
- [x] The live build number is discovered at startup, with a compiled-in fallback
- [x] Every REST request uses a persisted cookie jar and a persisted installation id
- [x] X-Discord-Locale, X-Discord-Timezone and a Referer that follows the open channel are sent
- [x] Session tests assert the IDENTIFY properties and the REST header set that mockcord observed

## Answer

Merged to main as `9052eac..1980a30`. Docs: `docs/fingerprint.md`.

- One module, `crates/protocol/src/fingerprint.rs`: `CHROME_MAJOR` (152), `FALLBACK_BUILD_NUMBER` (647615), `API_VERSION` (9), `Fingerprint` (UA, properties, locale, timezone, build number, base64 super-properties, Accept-Language), `IdentifyProperties` (the IDENTIFY `properties`, identical to decoded `X-Super-Properties`), `sec_ch_ua()`, `channel_referer()`. Fields: os, browser, device, system_locale, browser_user_agent, browser_version, os_version ("10"), referrer, referring_domain, referrer_current, referring_domain_current, release_channel, client_build_number, client_event_source (null), has_client_mods (false), client_launch_id, client_app_state ("focused"), client_heartbeat_session_id. `launch_signature` is omitted (only known from concord's code).
- Build-number scrape in `rest::api::build` (`discover_build_number`, https discord.com or loopback only, size-capped, same UA and cookie jar). IDENTIFY waits for it via `Gateway::spawn_when`, capped at 5 s, silent fallback. Under mockcord it reads `Server::app_url()`.
- `rest::ClientIdentity` = fingerprint + `CookieJar` + installation id, shared by login and the session; `RestClient::set_referer`. Installation id and persistent cookies live in the encrypted `kv` table. The installation id is adopted from READY.
- API changes: `SessionConfig` has `identity` and `build_source` instead of `identify`/`locale`; `TokenLogin::validate`, `session::login` take `&ClientIdentity`; `QrConfig` has `identity`; `RestConfig` has no `Default`. Apps build one `session::client_identity(&store)` per launch and pass `build_source: Some(discord_build_source())` against real Discord.
- Harness: `Running::restart()` (same store), `Options::transform_ready()`; launch settings kept across restart. Fixed: the harness and xtask opened the store before enabling plaintext secrets, writing a cache key to the real Credential Manager.
- For 04 (real traffic): verify Chrome major 152, fallback build 647615, the scrape patterns (`build_number:"`, `buildNumber:"`, `buildNumber","`), the hard-coded sec-ch-ua GREASE, `Priority: u=1, i`, field order, and that `X-Debug-Options`, `X-Fingerprint`, a forced `locale` cookie are absent.
