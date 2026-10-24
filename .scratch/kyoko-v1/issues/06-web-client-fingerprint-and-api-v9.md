# 06 — Web-client fingerprint and API v9

**What to build:** kyoko presents one consistent Discord web-client identity on both the gateway and REST, and uses API v9 like the web client.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** claimed

- [ ] REST and the gateway use API v9
- [ ] A reduced Chrome user agent (<major>.0.0.0) and complete super-properties are used, consistent between gateway and REST
- [ ] The live build number is discovered at startup, with a compiled-in fallback
- [ ] Every REST request uses a persisted cookie jar and a persisted installation id
- [ ] X-Discord-Locale, X-Discord-Timezone and a Referer that follows the open channel are sent
- [ ] Session tests assert the IDENTIFY properties and the REST header set that mockcord observed
