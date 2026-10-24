# 03 — Spike A: zgui window with a live server/channel list

**What to build:** A kyoko window opens on Windows. Its server and channel list is drawn in Luau and updates live from the session through the event bridge. It works against both mockcord and real Discord.

**Blocked by:** 02 (Strip GPUI UI, voice, WASM plugins and updater)

**Status:** claimed

- [x] zgui is pinned to a recorded commit, and the workspace carries zgui's required crate patches
- [x] The desktop app opens a zgui window on Windows and hot-reloads its Luau scripts in development
- [x] Session events cross to the UI thread through an unbounded inbox drained by a UI-thread task, not zgui's latest-value channel
- [x] A Luau list shows the servers and the selected server's channels, and updates live as mockcord's state changes
- [ ] With a token from KYOKO_TOKEN, the same list shows the throwaway account's real servers

## Comments

Code merged to main as `0e5d950..9ce1b0c`. Only the real-Discord box is open: `KYOKO_TOKEN` was not set. The real-Discord run is `cargo run -p kyoko` (no `--mock`) in the lane-a worktree, once, look and close; the token stays in memory (`SessionConfig::remember_account = false`).

- zgui pinned at `a9dc28246170b4099a414191ca6fc172586b0f9c` (`zgui`, `zgui-desktop`, `zgui-script`), with the nine `[patch.crates-io]` entries at the same rev, `allow-git` in `deny.toml`, `LUAU_CXXFLAGS=-DNDEBUG` in `.cargo/config.toml`. `luau0-src` must stay at `0.22.0+luau740` (held in `Cargo.lock`; `0.22.1+luau744` crashes at startup with "attempt to load a text chunk"). objc2 0.6.4 and blocking 1.6.2 are held for zgui's pins.
- Bridge: UI-free crate `crates/bridge`: `Inbox<T>` (mutex + waker), `changes_notifier(inbox)` as the session's `NotifyFn`, `Sidebar::apply(&State, &Changes) -> Refresh`, `Sidebar::select` → `Command::SubscribeGuild`. `apps/desktop/src/ui.rs` (`Link`) drains the inbox on zgui's UI thread and sets `guilds`, `channels`, `selected_guild`, `guild_name`, `status` inside `runtime.batch`. Luau `ui.action("select_guild", id)` → host action → `session.send`. The status line reads `SessionInfo::connection`.
- `kyoko --mock --churn N` makes mockcord add channels/guilds live (`mockcord::Server::create_guild`/`create_channel`).
- Measurements (debug, with another lane building): cold build about 19.5 min, warm full rebuild 12–13 min, warm incremental 4–5 s with link.exe; rust-lld not kept (not clearly faster, linker switch forces a full rebuild). Debug binary 54 MB, idle about 185 MB working set on mockcord.
- Snowflake ids cross to Luau as strings. To find the window from scripts use EnumWindows by title; `MainWindowHandle` can be a zgui helper window.
- zgx: zgui was renamed zgx (same repo `zeronsh/zgx`, MIT, still 0.1 from git, APIs kyoko uses unchanged, same nine patches, still `luau0-src 0.22.0+luau740`). The user decided to move to zgx at its current main before ticket 04, as part of finishing this ticket.
