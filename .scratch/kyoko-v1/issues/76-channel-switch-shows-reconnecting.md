# 76 — Opening a channel shows Reconnecting and reflows the sidebar

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Every time the user opens a new channel, kyoko shows a "Reconnecting" state while the messages load, then the channel list refreshes and the layout jumps. Messages do arrive. The user also asks whether kyoko loads messages as efficiently as Discord, and suspects it loads every author's profile when a channel opens. Discord isolates these: the message pane shows a skeleton while history loads, and nothing else on screen changes.

Evidence: the orchestrator ran kyoko with `--log info,gateway=debug` (logs under `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/`, no token in them; read event names only). Find out what actually shows the banner: a real gateway disconnect (close code), the connection banner misreading a REST or member-list state, or a loading state labelled wrongly.

**Blocked by:** none

**Status:** open

- [ ] Root cause written down (which state shows "Reconnecting" on a channel open, and why); fixed so the connection banner shows only for real gateway disconnects
- [ ] Opening a channel shows a message skeleton in the message pane only, until the first page arrives; the channel list, member list and server rail don't rebuild or move (a test asserts the sidebar state is unchanged by a channel open)
- [ ] Channel open sends what the web client sends and no more: one history page (`limit=50`), the guild subscription op if needed, no per-author profile fetches (profiles are 77); a session test counts requests per channel open
- [ ] A failure in one area (member list, history, profiles) leaves the others working; the message pane shows its own error with retry
- [ ] `docs/` updated with the channel-open sequence
