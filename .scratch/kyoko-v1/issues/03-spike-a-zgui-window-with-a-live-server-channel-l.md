# 03 — Spike A: zgui window with a live server/channel list

**What to build:** A kyoko window opens on Windows. Its server and channel list is drawn in Luau and updates live from the session through the event bridge. It works against both mockcord and real Discord.

**Blocked by:** 02 (Strip GPUI UI, voice, WASM plugins and updater)

**Status:** claimed

- [ ] zgui is pinned to a recorded commit, and the workspace carries zgui's required crate patches
- [ ] The desktop app opens a zgui window on Windows and hot-reloads its Luau scripts in development
- [ ] Session events cross to the UI thread through an unbounded inbox drained by a UI-thread task, not zgui's latest-value channel
- [ ] A Luau list shows the servers and the selected server's channels, and updates live as mockcord's state changes
- [ ] With a token from KYOKO_TOKEN, the same list shows the throwaway account's real servers
