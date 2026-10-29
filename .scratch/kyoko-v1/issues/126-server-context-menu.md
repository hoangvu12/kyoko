# 126 — Server context menu like Discord's

**What to build:** From the user's real run 4 (2026-10-30), image-06 in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy). Right-clicking a server icon on the rail opens Discord's server menu: Mark As Read, Invite to Server, Mute Server > (durations), Notification Settings > (All Messages / Only @mentions / Nothing, suppress @everyone and roles, etc.), Hide Muted Channels, Show All Channels, Privacy Settings, Edit Per-server Profile, Move to... > (folders), Leave Server (with Discord's confirmation), Copy Server ID (with developer mode, as Discord does). Reuse what exists: paced acks (10), invite dialog (35), per-server profile (81), guild settings proto and folders (22), menus as zgx portals (Midnight 07). Each action sends exactly what the web client sends (docs.discord.food; write the requests in `docs/`); risky ones (Leave) go through the lock rules (11). Entries kyoko can't do yet open the closest existing page, never a dead item.

**Blocked by:** 125 (same rail files)

**Status:** ready-for-agent

- [ ] Right-click on a server icon (and inside a folder) opens the menu with Discord's entries, order and submenus
- [ ] Mark As Read, Mute Server, Notification Settings, Hide Muted Channels / Show All Channels, Leave Server, Copy Server ID work; session tests at the mockcord seam prove each request (count and shape)
- [ ] Invite, Privacy Settings, Edit Per-server Profile, Move to... open kyoko's existing dialogs/pages
- [ ] In-process UI test for opening and choosing; capture checked by eye against image-06
