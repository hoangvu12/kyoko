# 76 — Opening a channel shows Reconnecting and reflows the sidebar

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Every time the user opens a new channel, kyoko shows a "Reconnecting" state while the messages load, then the channel list refreshes and the layout jumps. Messages do arrive. The user also asks whether kyoko loads messages as efficiently as Discord, and suspects it loads every author's profile when a channel opens. Discord isolates these: the message pane shows a skeleton while history loads, and nothing else on screen changes.

Evidence: the orchestrator ran kyoko with `--log info,gateway=debug` (logs under `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/`, no token in them; read event names only). Find out what actually shows the banner: a real gateway disconnect (close code), the connection banner misreading a REST or member-list state, or a loading state labelled wrongly.

**Blocked by:** none

**Status:** resolved

- [x] Root cause written down (which state shows "Reconnecting" on a channel open, and why); fixed so the connection banner shows only for real gateway disconnects
- [x] Opening a channel shows a message skeleton in the message pane only, until the first page arrives; the channel list, member list and server rail don't rebuild or move (a test asserts the sidebar state is unchanged by a channel open)
- [x] Channel open sends what the web client sends and no more: one history page (`limit=50`), the guild subscription op if needed, no per-author profile fetches (profiles are 77); a session test counts requests per channel open
- [x] A failure in one area (member list, history, profiles) leaves the others working; the message pane shows its own error with retry
- [x] `docs/` updated with the channel-open sequence

## Answer

Merged to main as `9b8473d` (worker `b16080dc`, lane A; checks re-run by the orchestrator: all green, parity channel-sidebar 17.5%, timeline 17.5%; after rebasing onto 79 and 88, clippy, bridge 210, session 241 and xtask test re-run green). Docs: `docs/gateway.md` ("Opening a channel", banner rule), `docs/ui.md` (`sidebar.rows`/`sidebar.badges`).

- "Reconnecting": the only such text is the connection banner, which reads only the gateway status; in the logged real run the gateway never left Connected, so the exact sighting was not reproduced. Real mislabel fixed: after the first READY, a "connecting"/"resuming" status on the same open socket (op 9 Invalid Session re-identify, 1–5 s) read "Reconnecting..." and the new READY rebuilt the channel list. Now "Reconnecting" only follows a real socket close (`crates/bridge/src/connection.rs`); test `a_fresh_identify_on_an_open_socket_shows_no_banner`.
- Reflow, two causes: (1) zgx's keyed `ui.each` treats a new allocation with equal contents as changed, and the sidebar, rail and member list published fresh values every time, so the read ack after any open rebuilt every row (lost hover, replayed slide animations). New `apps/desktop/src/stable.rs` (`StableRows`) reuses unchanged values; unread/mention state moved out of `sidebar.rows` into `sidebar.badges`. (2) THREAD_LIST_SYNC (reply to op 37 on the first open in a server) listed every active thread; Discord lists only joined ones. Threads first learnt from the sync stay out of the channel list unless joined or already known (threads popout still has them).
- Channel open sends one `GET /channels/{id}/messages?limit=50`, one op 37, and read acks; reopening a cached channel sends nothing; no `/users/` or profile requests (`a_channel_open_sends_one_page_and_one_subscription_and_nothing_else`, helper `assert_only_history_and_acks` for 77). A failed first page shows the reason and Retry in the pane only (`Command::RetryHistory`, `MessageRow::Failed`); gateway and member list unaffected.
- Mockcord: answers op 37 with THREAD_LIST_SYNC; `add_unjoined_thread` control (READY still includes unjoined fixture threads, which real Discord doesn't).
- For later: route any Rust list published to `ui.each` through `StableRows` (the DM sidebar and friends list still don't). An op 9 still reloads all state, but rows keep their values through it.

