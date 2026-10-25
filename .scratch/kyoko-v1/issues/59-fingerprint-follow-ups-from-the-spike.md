# 59 — Fingerprint follow-ups from the spike

**What to build:** The corrections ticket 04 found when it compared kyoko's IDENTIFY and REST traffic with the current Discord web client. kyoko keeps presenting one consistent web-client identity.

**Blocked by:** 13 (Versioned read state, persisted client state and late DMs)

**Status:** ready-for-agent

- [ ] The compiled-in Chrome major matches current stable (156 at the time of the spike) and the `sec-ch-ua` brand string is recomputed for it, in the one fingerprint module
- [ ] IDENTIFY follows the web client's key order and drops `large_threshold`; presence is sent as `"unknown"` with `since: 0`
- [ ] `client_state` carries `api_code_version`, echoing READY's value when known
- [ ] `installation_id` is also sent in IDENTIFY where the web client sends it
- [ ] `client_heartbeat_session_id` rotates on the web client's schedule (every 30 minutes per the docs)
- [ ] `launch_signature` is added only if its format is documented in public protocol docs; if the only source is a GPL client, it stays omitted and the reason is recorded in `docs/fingerprint.md`
- [ ] Session tests assert each change on the wire

Source: `docs/spike.md` (real-traffic comparison table, sources discord.food / userdoccers read 2026-10-25). The fallback build number was already updated to 649389 in ticket 04.
