# 77 — Load user profiles when a profile is opened, not with the channel

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). The user believes profiles are fetched when a channel opens (profile popouts show their data at once). Discord fetches `GET /users/{id}/profile` only when a popout or full profile opens (with `with_mutual_guilds`, `guild_id`), shows a skeleton while it loads, and caches it.

**Blocked by:** none

**Status:** claimed

- [ ] Find what kyoko fetches on channel open for authors; no profile request goes out until a profile popout, full profile or member-list click opens it (session test)
- [ ] The popout shows what's already known (name, avatar, roles from the member) at once and fills in bio, banner and connections when the profile arrives; a short cache avoids refetching while it's fresh
- [ ] Profile requests are paced and deduplicated (opening the same popout twice sends one request)
