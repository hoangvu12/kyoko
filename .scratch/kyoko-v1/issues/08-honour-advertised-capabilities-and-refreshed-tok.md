# 08 — Honour advertised capabilities and refreshed tokens

**What to build:** kyoko only asks Discord for behaviours it handles, and keeps the stored token current when Discord refreshes it.

**Blocked by:** 05 (Session test harness on mockcord)

**Status:** resolved

- [x] The advertised capability set contains only bits whose effects are implemented, guided by the spike findings
- [x] A refreshed token delivered on connect replaces the stored one
- [x] If batched reactions or passive guild updates are advertised, their dispatches are parsed and applied
- [x] Session tests cover the refreshed token and the effect of each advertised capability

## Answer

Merged to main as `d4a7101..dc93a2f`. Bit table in `docs/gateway.md`.

- IDENTIFY advertises `509` (was 32767): bits 0 LAZY_USER_NOTES, 2 VERSIONED_READ_STATES, 3 VERSIONED_USER_GUILD_SETTINGS, 4 DEDUPE_USER_OBJECTS, 5 PRIORITIZED_READY_PAYLOAD, 6 MULTIPLE_GUILD_EXPERIMENT_POPULATIONS, 7 NON_CHANNEL_READ_STATES, 8 AUTH_TOKEN_REFRESH. Dropped: 1, 9 (settings proto), 10, 11/14 (passive guild updates, not parsed), 12, 13 (reaction ADD_MANY, not parsed), 15, 16. Bits are named in `protocol::capability`; the set is the one constant `gateway::ADVERTISED_CAPABILITIES`. Ticket 04 compares it with real traffic.
- Refreshed token: the gateway swaps READY's `auth_token` into `GatewayConfig.token` before handling READY and emits `GatewayEvent::TokenRefreshed`; the session gives it to REST before applying READY. It is written to the store only when `remember_account` is true. Saves and sign-out share a signed-in lock, so a save after sign-out writes nothing.
- Mockcord accepts one token at a time (the refreshed one replaces the old), strips `auth_token` unless bit 8 is advertised, shapes READY by bits 0/2/3/4/5/7, and sends MESSAGE_REACTION_ADD_MANY only with bit 13.
- Harness: `running.store()`, `running.eventually(..)` (10 ms polling for effects that don't change state), `Options::remember_account(bool)`. 9 tests in `tests/session/capabilities.rs`. Fixed a flaky hook test.
- For 04: check whether real READY carries `auth_token` and whether the old token then stops working; record real ADD_MANY and passive-update shapes before enabling those bits.
- For 13: without the passive bits Discord sends CHANNEL_UNREAD_UPDATE for unsubscribed guilds; it isn't parsed yet, so their unread state can go stale.
- Known flake: 07's `a_bulk_burst_stays_inside_the_send_window_while_heartbeats_flow` fails occasionally under heavy parallel builds; being fixed on lane C.
