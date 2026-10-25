# 45 — Plugin runtime with ClearURLs

**What to build:** Users can enable, configure and disable Luau plugins, approving each one's permissions explicitly. ClearURLs, the first plugin, strips tracking parameters from outgoing links.

**Blocked by:** 27 (Composer), 37 (Account and settings pages)

**Status:** resolved

- [x] Each plugin declares a manifest (permissions, settings schema, version), and enabling it asks the user to approve its permissions
- [x] The plugin's logic runs in a sandboxed Luau VM on a worker thread with call deadlines. Repeated errors or overruns disable the plugin with a notice and never crash kyoko
- [x] Plugins get their own storage and settings. Shared Discord data is read-only to them
- [x] Discord actions from plugins are refused outside a user gesture, and rate-limited per plugin
- [x] The plugin settings screen follows oxidcord's visual style, and plugins hot-reload in development
- [x] The before-send hook exists. ClearURLs ships its rules inside the package and works
- [x] Session tests cover permissions, the gesture rule, deadlines and the hook

## Comments

From the spike (04): zgx host image names and shared-value names are global, so plugins need their own namespace (prefix per plugin id) to avoid clashing with the app and each other.

## Answer

Merged to main as `b1b5589..65b8ae8` (the worker's non-building hook-signature commit squashed into the runtime commit by the orchestrator, same tree). Docs: `docs/plugins.md` (the plugin API v1 contract every plugin ticket builds on), `docs/architecture.md`, `docs/ui.md`, `docs/parity.md`, `docs/testing.md`, `THIRD_PARTY_NOTICES.md`.

- New crate `crates/plugins` (`PluginHost`, no UI dependency, builds headless), attached to the session only through `SessionHooks`. One sandboxed Luau VM per enabled plugin on its own worker thread: `Lua::new()` + `sandbox(true)`, `print` removed, memory limit, per-call deadline enforced by the VM interrupt (a `pcall` can't swallow it) and by the caller, `require` limited to flat `.luau` file names inside the plugin. Per-plugin storage and settings files; events filtered by read permissions; state handed out as copies (read-only).
- Manifest `plugin.toml`: permissions, settings schema, version. Enabling opens a prompt that grants all shown permissions or nothing; a newer version asking for more stops until re-approved. Reserved permissions (`network:*`, display/timestamp/emoji/render hooks, upload/activity actions) parse and show as "Not used yet" but grant nothing; v1 has no network API.
- Failure policy: three errors or overruns suspend the plugin with a red notice bar under the title bar; restart after 30 s, then 2 min; the third suspension turns it off until the user re-enables it.
- Gesture rule: plugin Discord actions run only right after a real click or key press (`PluginHost::gesture`), rate-limited per plugin, and go out as the same session `Command` a user click sends (locks and pacing apply). `kyoko.actions.mark_all_read()` (`action:mark_read`) exists on the paced bulk-ack path.
- Session change: `before_send` returns a `Rewrite` naming the plugin that changed the message. No cancel in before-send (spec asks only for rewriting).
- ClearURLs (`plugins/clear-urls/`, compiled into the binary, ships disabled): kyoko's own rule set (the upstream ClearURLs rules are LGPL-3.0, not bundled), cleans links in sent and edited messages, skips code spans and blocks, "Remove referral tags" setting, never fetches.
- Desktop: Plugins page in a new "Kyoko" settings section in oxidcord's style (Catalog tab, Install from File, safe mode, Uninstall and partial grants left out; "Open Plugins Folder" instead); per-plugin settings page generated from the manifest; `--plugins-dir` flag; hot reload of the user plugins folder and `--plugins-dir` (debug builds or when the flag is given). `plugin.<id>.*` names are deliberately outside 60's load check; the Plugins page and notice bar are inside it.
- UI half: only the generated settings pane exists. The other slots (`user.decoration`, `member.decoration`, `message.accessory`, `composer.buttons`, `panel.buttons`, `rail.buttons`, context-menu items) are named and documented but draw nothing; the loader for a plugin's own `ui.luau` is described step by step in `docs/plugins.md` for 47 to build.
- `cargo xtask headless-run --mock` now runs the real host with ClearURLs and a deliberately failing test plugin.
- Tests: 13 in `crates/plugins/tests/plugins/` driving a session against mockcord (approval, re-approval on new permissions, deadlines including a `pcall`-wrapped endless loop, storage/settings ownership, refused ungranted APIs and read-only data, gestures and per-plugin rate limit, hot reload, ClearURLs), included in `cargo xtask test`; Plugins page approval flow in the load check. Parity (port 8494, 150% scale): settings-plugins 3.0% (oxidcord lists no plugins on a fresh data dir). Checks re-run by the orchestrator in lane A.
- For 46: `before_send`/`before_edit` with `hook:send`; add a `pairs` setting type (list of `{from, to}`) and its editor. 47: `allow_typing` with `hook:typing` works now; build the UI-half loader and the `composer.buttons` slot. 48: add `hook:display_name`, `hook:timestamp`, cached per revision, never blocking the UI thread. 49: `user.decoration`/`member.decoration` slots, presence via `read:users` (see 30's note on `client_status`). 50: add `hook:emoji_rank` off the UI thread (32's `bridge::EmojiPicker`). 51: `allow_notification` with `hook:notify` and `read:dms`, `message_ack` events (`read:read_state`). 52: `hook:message_render` and the `message.accessory` slot drawn in Rust. 53: `action:upload_expression`; menu items in `bridge::message_menu` and `MessageActions::menu_items(entry, access)` (timeline, hover More menu and search), clicks through `PluginHost::gesture`. 54: `rail.buttons` slot plus `kyoko.actions.mark_all_read()`. 55: display-only, `read:guilds`, no fetches; the hidden-channel lookup is not built yet. 56: `action:activity` and the `panel.buttons` slot. 57: put Themes in the "Kyoko" settings section.
