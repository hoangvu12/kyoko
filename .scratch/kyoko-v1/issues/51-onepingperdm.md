# 51 — OnePingPerDM

**What to build:** Repeated DMs from the same person ping only once until they're read.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 40 (Tray, notifications and desktop integration)

**Status:** resolved

- [x] A before-notify filter hook exists
- [x] OnePingPerDM suppresses repeat sounds and notifications until the DM is read

## Answer

Merged to main as `5138238..3a4b549`. Docs: `docs/plugins.md`, `docs/desktop.md`, `docs/architecture.md`, `docs/parity.md`.

- Before-notify hook: `allow_notification` with `hook:notify` already existed from 40/45 (100 ms deadline, fails open). Changed: the session reserves the alert's slot in the burst limit (3 per 10 s) before asking the hook and gives it back on a veto, so the hook only hears alerts that will actually show (`the_hook_hears_only_alerts_the_burst_limit_would_show`). Calls for separate messages run concurrently and can finish in any order. A veto silences the Windows notification and its sound, nothing else. The hook's message now has `guild_id` filled from state when the gateway omits it. It isn't told the alert kind; a later plugin that needs it adds a `kind` field (small Rust signature change).
- The documented plugin `ready` event is now delivered after each live READY (new connection or another account, not a resume): `every_plugin_hears_who_signed_in`. Plugins clear in-memory state on `ready` when `user_id` changes.
- `kyoko.state.channel` reads a DM or group DM with `read:dms` alone (server channels still need `read:guilds`).
- OnePingPerDM (`plugins/one-ping-per-dm/`, bundled, ships disabled; `hook:notify`, `read:dms`, `read:read_state`): remembers per channel the last message that notified; later ones are silenced until the read state reaches it (marked read, viewed with the window focused, or read elsewhere). Settings: Limit (all DMs default, one-on-one only, group DMs only) and "Also limit server channels" (off; covers every server notification in a channel). Unread counts, mention badges, the tray badge and in-window toasts are unchanged. Vencord's DM mention exceptions left out (a DM mention already notifies as a DM).
- Tests: `crates/plugins/tests/plugins/one_ping_per_dm.rs` (three DMs notify once; reading or viewing lets the next notify; group vs one-on-one; server mentions untouched by default and limited when on; plugin off; failing or looping filter fails open); `headless-run` checks one notification for three DMs and another after reading. Checks re-run by the orchestrator in lane C, all green.
- Tests that open the plugins page should load only the plugin under test (each bundled plugin adds a card; TextReplace's editor test now does).
