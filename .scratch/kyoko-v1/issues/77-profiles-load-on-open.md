# 77 — Load user profiles when a profile is opened, not with the channel

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). The user believes profiles are fetched when a channel opens (profile popouts show their data at once). Discord fetches `GET /users/{id}/profile` only when a popout or full profile opens (with `with_mutual_guilds`, `guild_id`), shows a skeleton while it loads, and caches it.

**Blocked by:** none

**Status:** resolved

- [x] Find what kyoko fetches on channel open for authors; no profile request goes out until a profile popout, full profile or member-list click opens it (session test)
- [x] The popout shows what's already known (name, avatar, roles from the member) at once and fills in bio, banner and connections when the profile arrives; a short cache avoids refetching while it's fresh
- [x] Profile requests are paced and deduplicated (opening the same popout twice sends one request)

## Answer

Merged to main as `57f8d96` (worker `57774ff0`, lane A; checks re-run by the orchestrator: all green, parity profile-popout 20.2%; after rebasing onto 86, clippy and xtask test green). Docs: `docs/rest.md` ("Profiles", `profile_gap`), `docs/gateway.md` points there.

- Already true before: `GET /users/{id}/profile` only goes out when the popout, the full profile or a DM's profile column opens (kept: Discord's DM panel fetches it too). Nothing fetches on channel open, member hover, prewarm or READY; the only other `/users/` request is backfill's `GET /users/{id}` for unknown DM mentions. The data looks instant because the popout draws the known user and member at once and profiles are cached 5 minutes. The query already matches docs.discord.food (`with_mutual_guilds`, `with_mutual_friends`, `with_mutual_friends_count`, `guild_id`); the web client's `type=popout` analytics field is left out.
- Fixed: a loading state (`State::profile_loading`, `ProfileCard.loading`, placeholder bars via `profile::parts::placeholder(widths)` for About Me/Bio and Connections, then "Nothing to show here yet."); transient failures are no longer cached (404/403 still are), so reopening retries; profile reads get their own pacing lane, 500 ms apart (`profile_gap`), independent of writes.
- Tests: `profiles_load_only_when_opened_once_while_fresh_and_paced` (uses 76's `assert_only_history_and_acks`), `a_failed_profile_stops_loading_and_the_next_open_tries_again`, bridge `an_opened_profile_shows_the_member_at_once_and_fills_in_when_it_arrives`.
- For 81: `State::profile(user_id)` stores one profile per user although the session cache is keyed per user and guild; server profiles need per-guild storage (or `guild_member_profile` from the `guild_id` response). Use `loading` for banner/avatar skeletons. For 92: `bridge::dm_panel` still reads `card.loaded` and shows no loading state. Route every new profile view through `Command::LoadProfile`.

