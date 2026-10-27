# 80 — Opening a server opens its last or default channel

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Clicking a server in kyoko shows a blank message area. Research what the Discord web client does (public sources and docs.discord.food; clean-room): it reopens the channel last viewed in that server, else the server's default channel (first readable text channel by position, or the home/rules channel where onboarding is on), and remembers it per server across restarts.

**Blocked by:** none

**Status:** resolved

- [x] Behaviour written in `docs/` with sources
- [x] Opening a server opens its last viewed channel, else the default one; remembered per account across restarts
- [x] A session or bridge test covers last viewed, first visit, and a last-viewed channel that was deleted or became unreadable

## Answer

Merged to main as `6941fbe` (worker `fb92097c`, lane C; checks re-run by the orchestrator: all green, parity server-rail 14.2%; rebased onto 77 with one union conflict in `crates/state/src/state.rs` fields, full checks re-run green). Docs: `docs/ui.md` ("Opening a server", sources docs.discord.food User Settings Proto and Guild), storage docs (kv key).

- Root cause: selecting a server only subscribed to it and closed the timeline; nothing chose a channel, and there was no per-server memory.
- `bridge::landing_channel`: the last viewed channel if it can still be opened (in the server's channel tree, viewable, text/announcement/voice/stage/thread/forum), else the first viewable text or announcement channel in sidebar order (same rule as `replacement_channel`). `Link::visit_guild` for rail clicks, the quick switcher and newly joined servers; `Link::follow_selection` at launch and when the selected server changes on its own, so launch no longer shows a blank area. Paths that open a specific channel use `select_guild` without landing (one history page and one op 37 per open, as 76 requires; subscriptions are debounced so a server click sends one op 37).
- Memory: state records server -> channel on every guild channel select (not DMs); the session saves the pairs in kv `navigation.last_channels` in the account's database and loads them before the cached READY; bad entries are skipped one by one, left servers dropped, sign-out wipes it. `State::last_channel`/`last_channels` are public. Discord keeps this on the device (no settings-proto field), so kyoko syncs nothing.
- No Server Guide/home view in kyoko: onboarding servers also open their first readable text channel; rules is never the default. Archived and unjoined threads fall back to the default.
- Tests: bridge `crates/bridge/tests/bridge/landing.rs` (first visit/last viewed/restart, deleted channel, lost View Channel), session `crates/session/tests/session/navigation.rs` (saved, restored, bad entry skipped).
- For later: call `Link::visit_guild` for "go to this server", `select_guild` only when opening a specific channel right after. A future home view goes into `landing_channel` ahead of the fallback.

