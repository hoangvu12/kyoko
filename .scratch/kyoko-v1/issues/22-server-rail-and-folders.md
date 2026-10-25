# 22 — Server rail and folders

**What to build:** The server rail matches oxidcord: servers, folders, unread and mention indicators, and reordering.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] The rail, folders and indicators match oxidcord in the parity captures
- [x] Reordering servers and folders persists to Discord
- [x] Unread and mention indicators update live

## Answer

Merged to main as `fae62a3..8733e35`. Docs: "Server order and folders" in `docs/rest.md`; `docs/ui.md`, `docs/media.md`.

- Luau port of oxidcord's rail (`ui/shell/server_rail.luau`, components `rail_tile`, `unread_pill`, `mention_badge`, `rail_gap`): home button with DM mention total, servers and folders in layout order, add-server/discover, tooltips, animated unread pill (8/20/40 pt), circle↔rounded-square morph, mention badge, folders (open: tinted folder icon; closed: grid of up to 4 mini icons + combined counts). Selecting a server opens its folder.
- Projection `bridge::Rail` (`crates/bridge/src/rail.rs`) publishes `rail.items` (guild/folder entries), `rail.home_mentions`, `rail.open_folders` (store kv `rail.open_folders`) via `apps/desktop/src/rail.rs`; muted servers don't show unread. Pure reorder logic `rearranged`, `rearranged_folder`, `reorder_command`. Actions `rail.drop`, `rail.toggle_folder`, `rail.select_home` (stub, 24), `rail.add_server` (stub, 35), `rail.discover`. `rail.guilds` is gone.
- Reorder: drag before/onto (new folder)/into/out of folders/to the end; folders draggable too (beyond oxidcord, as the ticket asks); no nesting. Persisted with `PATCH /users/@me/settings-proto/1` `{"settings":"<base64>"}` carrying only field 14 `guild_folders` (`protocol::encode_guild_folders`), whole field each time, ungrouped servers as id-less one-server folders. Session writer `task/layout.rs`: applied locally at once, one request 1 s after the last change (`settings_delay`), ≥10 s apart (`settings_gap`), mutation lane; a failed write rolls back and reports a rejection; an unsent change is lost on quit. Capability bit 9 stays off (the proto arrives in READY and USER_SETTINGS_PROTO_UPDATE anyway). Mockcord decodes and stores the write.
- Icons: one `Square` gallery image per guild; a zgx node with `rounded` plus `overflow = "hidden"` clips images, so no pre-masking (20's note corrected in docs).
- Parity: rail region 7.56% differing, almost all icon artwork (oxidcord's capture mode routes CDN URLs to mockcord `/media/<hash>.png`, kyoko uses the new `/cdn/icons` route); whole window 10.1%.
- Tests: 4 in `tests/session/layout.rs`, 2 in `bridge/tests/bridge/rail.rs`, 11 pure reorder tests, 3 settings-proto encoder tests; harness `Options::guilds(n)`.
- To confirm on a real run: that the settings-proto PATCH with only `guild_folders` merges and leaves other settings intact on real Discord.
- Known load flake seen during verification: `capabilities::a_refreshed_token_reaches_rest_the_gateway_and_the_store` timed out once under heavy parallel load; 6/6 in isolation.
- For 23: selection is still `sidebar.guild_id` / `rail.select_guild`. For 24: Home is selected when `sidebar.guild_id == ""`; wire `rail.select_home`. For 35: `rail.add_server` stub; `rail_tile` takes a `context_menu` prop. For 54: `Command::MarkAllRead`.
