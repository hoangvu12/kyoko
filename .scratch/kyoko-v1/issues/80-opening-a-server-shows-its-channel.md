# 80 — Opening a server opens its last or default channel

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Clicking a server in kyoko shows a blank message area. Research what the Discord web client does (public sources and docs.discord.food; clean-room): it reopens the channel last viewed in that server, else the server's default channel (first readable text channel by position, or the home/rules channel where onboarding is on), and remembers it per server across restarts.

**Blocked by:** none

**Status:** open

- [ ] Behaviour written in `docs/` with sources
- [ ] Opening a server opens its last viewed channel, else the default one; remembered per account across restarts
- [ ] A session or bridge test covers last viewed, first visit, and a last-viewed channel that was deleted or became unreadable
