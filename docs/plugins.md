# Plugins

Kyoko runs Luau plugins modelled on Vencord's hook surface. The contract is owned by the core and versioned: this page is **plugin API v1**. Tickets 46 to 57 build on it, and a change to anything here bumps the version.

The runtime is `crates/plugins` (`PluginHost`). It has no UI dependency and builds headless. It plugs into the session only through the optional `SessionHooks` interface (`docs/architecture.md`), so the session knows nothing about plugins, and a plugin failure never stops it.

## The two halves

- **Logic half** (`logic.luau`): data hooks, events, storage and actions. Each enabled plugin gets its own sandboxed Luau VM on a worker thread of its own, with a call deadline on every hook, so a per-message hook never runs on the UI thread or blocks the session for long. This half is built.
- **UI half** (`ui.luau`): a zgx plugin contributing to named UI slots, reaching nothing of the app's but the `plugin.<id>.*` values and actions the app exposes to it. It runs on the UI thread in its own sandboxed zgx VM, mounted while the plugin runs. The placed slots are `composer.buttons`, `panel.buttons` and `rail.buttons`; the settings pane is generated from the manifest and needs no UI code. See "UI slots".

## Where plugins come from

| Origin | Where | Notes |
|---|---|---|
| Bundled | `plugins/<id>/` in this repository, compiled into the binary (`crates/plugins/src/bundled.rs`) | Ships disabled. First-party only. |
| User | `<data dir>/plugins/<id>/` (Settings, Plugins, "Open Plugins Folder") | May not reuse a bundled plugin's id; such a folder is skipped with a warning. |
| Development | each `--plugins-dir <path>` (a plugin folder, or a folder of plugin folders) | Overrides a bundled or user plugin with the same id, so `--plugins-dir plugins` edits ClearURLs live. |

A package is a flat folder: `plugin.toml`, `logic.luau`, optional `ui.luau`, any `.luau` modules either half requires, and the images its UI half shows. Files are read as source, never bytecode, and are capped at 2 MiB each.

**Hot reload.** In debug builds, and whenever `--plugins-dir` is given, the host checks the user and development folders every 500 ms. A changed file restarts that plugin's VM with the new code (its storage and settings are kept) and mounts its UI half again; a removed folder unloads it; a manifest that asks for a permission not yet approved stops it until the user approves again. Bundled packages never change while kyoko runs.

## Manifest (`plugin.toml`)

```toml
id = "clear-urls"
name = "ClearURLs"
version = "1.0.0"
api = 1
description = "Strips tracking parameters from the links you send and edit."
authors = ["Kyoko"]
permissions = ["hook:send"]
logic = "logic.luau"
ui = "ui.luau"

[[settings]]
key = "referrals"
type = "boolean"
label = "Remove referral tags"
description = "Also remove affiliate and referral tags."
default = true
```

| Field | | |
|---|---|---|
| `id` | required | lowercase letters, digits and `-`, starting with a letter, at most 48 characters; also the plugin's namespace |
| `name` | | shown in settings and notices; defaults to the id |
| `version` | | free text, at most 64 characters |
| `api` | | the API version it was written for; anything but `1` loads as Incompatible and never runs |
| `description`, `authors` | | shown on the plugin's card |
| `permissions` | | names from the table below; duplicates are dropped, at most 32 |
| `logic`, `ui` | | entry files, default `logic.luau` and `ui.luau` |
| `[[settings]]` | | the settings schema, below |

Decoding is lenient, as everything else from disk: a bad setting is left out, an unknown permission is listed as unknown and never granted, unknown fields are ignored. A manifest without a valid id does not load.

### Settings schema

Each `[[settings]]` entry has `key` (lowercase letters, digits, `_`), `type`, `label`, `description` and, by type:

| `type` | Extra fields | Value | Shown as |
|---|---|---|---|
| `boolean` | `default` | `true`/`false` | a switch |
| `number` | `default`, `min` (0), `max` (100), `step` (1) | a number, stepped and clamped | a stepper |
| `string` | `default`, `multiline` | text up to 2000 characters | a text field, saved 500 ms after typing stops |
| `select` | `options` (strings, or `{ value, label }`), `default` | one of the option values | radio buttons |
| `action` | | none | a button that runs `on_action(key)` inside a user gesture |

The host validates every value (`SettingSpec::accept`); an invalid value changes nothing. Values live in `<data dir>/plugins.json` with the plugin's enabled flag and approved permissions. A list type for TextReplace's rules is not in v1: ticket 46 adds it (a `pairs` type: a list of `{ from, to }`), with its editor.

## Permissions

The user approves every permission the manifest asks for before the plugin runs, all or nothing: enabling a plugin opens the prompt, and "Allow" grants exactly the list it showed (`PluginHost::approve` refuses if the manifest changed under the prompt). A plugin whose new version asks for more stops with "Needs approval" until the user reviews it again; "Revoke Permissions" on its settings page clears the grant and turns it off.

| Permission | Grants | v1 |
|---|---|---|
| `storage` | `kyoko.storage` | yes |
| `read:messages` | message events (`message_create`, `message_update`, `message_delete`) in servers | yes |
| `read:dms` | extends every read to direct messages and group DMs (events, `kyoko.state.channel`) | yes |
| `read:users` | `kyoko.state.current_user`, `user`; `typing_start`, `presence_update` events | yes |
| `read:guilds` | `kyoko.state.guilds`, `channel`; `guild_create`, `guild_delete` events | yes |
| `read:read_state` | `kyoko.state.read_state`; `message_ack` events | yes |
| `hook:send` | `before_send` and `before_edit`: read and rewrite your own outgoing text, DMs included | yes |
| `hook:typing` | `allow_typing`: veto your typing indicator | yes |
| `hook:notify` | `allow_notification`: read messages that would notify you, and silence them | yes |
| `hook:display_name` | display-name formatting | reserved for 48 |
| `hook:timestamp` | relative-timestamp formatting | reserved for 48 |
| `hook:emoji_rank` | emoji autocomplete ranking | reserved for 50 |
| `hook:message_render` | message render access to kept deleted and edited versions | reserved for 52 |
| `action:mark_read` | `kyoko.actions.mark_all_read`, `mark_guild_read` | yes |
| `action:upload_expression` | emoji and sticker upload | reserved for 53 |
| `action:activity` | the activity-sharing toggle | reserved for 56 |
| `ui:composer.buttons`, `ui:panel.buttons`, `ui:rail.buttons` | show its UI half in that slot | yes |
| `ui:settings`, `ui:user.decoration`, `ui:member.decoration`, `ui:message.accessory`, `ui:message.menu` | contribute to that UI slot | reserved (see UI slots) |
| `network:<host>` | contact that host | reserved; v1 has no network API at all |

Reserved permissions parse, show in the prompt with "Not used yet", and grant nothing until their ticket adds the call path (`Permission::available`). The prompt marks sensitive ones (`hook:send`, `hook:notify`, `read:messages`, `read:dms`, actions, network). Granted permissions are `declared ∩ approved`.

Enforcement is in the host, never in the plugin: a hook is called only when its `hook:` permission is granted; every `kyoko.*` call checks its permission and raises a Luau error (`kyoko: kyoko.storage needs the storage permission`) when it is missing; events are filtered by their read scope before they reach the VM.

## Logic API v1

`logic.luau` runs once when the plugin starts and returns a table of hooks. Every hook is optional.

```lua
local rules = require("./rules")

return {
	before_send = function(message) return nil end,
	before_edit = function(message) return nil end,
	allow_typing = function(channel) return true end,
	allow_notification = function(message) return true end,
	on_event = function(event) end,
	on_action = function(key, context) end,
	on_settings_changed = function() end,
}
```

Ids are strings of digits. Every table the host passes is a fresh copy: writing to it changes nothing in kyoko.

### Hooks

| Hook | Permission | Called | Gets | May return | Deadline |
|---|---|---|---|---|---|
| `before_send(message)` | `hook:send` | before a typed message is sent, once per send | `{ channel_id, guild_id?, content }` | a string to replace the text, or `nil` | 250 ms |
| `before_edit(message)` | `hook:send` | before an edit is sent | `{ channel_id, guild_id?, message_id, content }` | as `before_send` | 250 ms |
| `allow_typing(channel)` | `hook:typing` | before each typing indicator (at most one per 9 s per channel) | `{ channel_id, guild_id? }` | `false` vetoes; anything else allows | 100 ms |
| `allow_notification(message)` | `hook:notify` | after kyoko's own notification rules, before the burst limit | `{ id, channel_id, guild_id?, author_id, author_name, content }` | `false` silences; anything else allows | 100 ms |
| `on_event(event)` | the event's read scope | after each listed gateway event is applied to state | see Events | nothing | 200 ms |
| `on_action(key, context)` | none | when the user clicks the plugin's `action` setting or one of its slot buttons | the key, and `{ source = "settings" }` or `{ source = "slot", slot = "composer.buttons" }` | nothing | 500 ms |
| `on_settings_changed()` | none | after the user changes one of its settings | nothing | nothing | 200 ms |

- Text hooks chain in plugin id order: each sees the previous plugin's output. A replacement that is empty, longer than 4000 characters or equal to the input counts as no change. A rewritten send is recorded as altered by the last plugin that changed it (`State::altered_by`). There is no "cancel" in v1: the spec asks for rewriting only.
- Vetoes stop at the first plugin that returns `false`.
- A failing or overrunning hook counts as a strike (below) and the session goes on with the unmodified value; a veto hook that fails allows.

### Events

`on_event` gets `{ type = ..., ... }`, never a raw gateway payload:

| `type` | Fields | Scope |
|---|---|---|
| `ready` | `user_id` | none |
| `message_create` | `channel_id`, `guild_id?`, `message` (as `allow_notification`'s) | `read:messages` (+ `read:dms` without `guild_id`) |
| `message_update` | `id`, `channel_id`, `guild_id?`, `content?` | as above |
| `message_delete` | `id`, `channel_id`, `guild_id?` | as above |
| `typing_start` | `channel_id`, `guild_id?`, `user_id` | `read:users` (+ `read:dms` without `guild_id`) |
| `presence_update` | `user_id`, `guild_id?`, `status` | `read:users` |
| `message_ack` | `channel_id`, `message_id` | `read:read_state` |
| `guild_create` | `guild_id`, `name` | `read:guilds` |
| `guild_delete` | `guild_id` | `read:guilds` |

Events queue per plugin (at most 256; more are dropped and logged) and are delivered in the order the store applied them. Hook calls jump the queue, so a busy event stream never delays a send by more than one event's deadline.

### The `kyoko` global

| Call | Permission | Returns |
|---|---|---|
| `kyoko.api` | | `1` |
| `kyoko.plugin.id`, `kyoko.plugin.version` | | the manifest's |
| `kyoko.log(...)` | | logs at debug level with the plugin id, clipped to 200 characters |
| `kyoko.settings.get(key)`, `kyoko.settings.all()` | | the current value (the default until changed); read-only |
| `kyoko.storage.get(key)`, `set(key, value)`, `delete(key)`, `keys()` | `storage` | values are nil, booleans, numbers, text and tables (16 levels deep); setting `nil` deletes |
| `kyoko.state.current_user()`, `user(id)` | `read:users` | `{ id, username, display_name, bot }` or `nil` |
| `kyoko.state.guilds()` | `read:guilds` | `{ { id, name } }` |
| `kyoko.state.channel(id)` | `read:guilds` (+ `read:dms` for DMs) | `{ id, guild_id?, name, type, parent_id? }` or `nil` |
| `kyoko.state.read_state(channel_id)` | `read:read_state` | `{ channel_id, last_message_id?, mention_count }` or `nil` |
| `kyoko.actions.mark_all_read()`, `mark_guild_read(guild_id)` | `action:mark_read` | nothing; raises if refused |
| `require("./name")` | | a module file of the same package, loaded once |

State queries read the session's `State` snapshot under its read lock and copy what they return, so shared Discord data is read-only to plugins. There is no API to write app state, fake client features, change the identity, reach REST or the gateway, or bypass a permission check. `print` is removed; the Luau standard library is the sandboxed one (`string`, `table`, `math`, `utf8`, `bit32`, `buffer`, `coroutine`, `os.time`/`clock`/`date`).

## Actions and the gesture rule

A Discord action runs only inside a user gesture:

1. The gesture starts in the app, never in a plugin: a click or key press on the plugin's button in kyoko's interface. The window records each pointer-down and key-down; `plugins.action` and a UI half's `plugin.<id>.action` and `plugin.<id>.set` are refused unless one happened in the last second, and consume it, so a script calling them in a loop or on a timer gets nothing.
2. The host calls `on_action(key, context)`; only during that call is `kyoko.actions.*` allowed, and only once per gesture. From any other hook or event it raises `kyoko: ... was refused: actions only run from your click or command`.
3. Each plugin may run at most 5 actions per 10 seconds (`Limits::actions`, `action_window`), across gestures and restarts.
4. An allowed action is a session `Command` (`MarkAllRead`, `MarkGuildRead`) sent through the same `Commander` as the user's own clicks, so it goes through the same locks, pacing and REST safety layer (`docs/locks.md`, `docs/rest.md`). Plugins never reach around them.

`PluginHost::gesture(id, key, context)` is the entry point; the desktop calls it only after the activation check above. The spec's other gesture, a command, will call the same entry point once plugins can register commands.

## Deadlines, failures and the disable policy

| Limit | Default (`Limits`) |
|---|---|
| send and edit hooks | 250 ms |
| typing and notification vetoes | 100 ms |
| events, `on_settings_changed` | 200 ms |
| `on_action` | 500 ms |
| loading `logic.luau` | 1 s |
| VM memory | 16 MiB |
| storage | 1 MiB and 1024 keys per plugin |

- The deadline is enforced inside the VM (Luau's interrupt raises once it passes, and keeps raising, so `pcall` cannot swallow it) and again by the caller, which stops waiting after twice the deadline plus 250 ms whatever the worker does. The session is never held longer than that per plugin.
- Each error, overrun, full queue or dead worker is a strike. Three strikes within 60 seconds (or a worker that stopped answering) suspend the plugin: its VM is dropped, it is skipped by every hook, and a notice says why and when it comes back. It restarts after 30 s, then 2 min (backoff ×4).
- The third suspension in a run turns it off: it stays disabled (saved) until the user turns it on again.
- A plugin that fails to load is shown as Failed with the error, and nothing else is affected.
- Notices show on the plugin's card, as a banner on the Plugins page, and in a bar under the title bar ("Open Plugins", dismiss) while settings are closed. Logs carry the plugin id and the kind of fault only; a plugin's error text is logged at debug level, never the message content.

## Storage and namespacing

- Storage: `<data dir>/plugin-data/<id>/storage.json`, written atomically after each call that changed it.
- Settings, enabled flag and approved permissions: `<data dir>/plugins.json`.
- zgx host images and shared values are global, so every name a plugin's UI half sees is under `plugin.<id>.*` (`plugin.clear-urls.settings`), and its zgx plugin name is its id. The app's own scripts use only the literal `plugins.*` names (`plugins.list`, `plugins.notice`, `plugins.folder`, and the `plugins.enable`, `disable`, `approve`, `revoke`, `reload`, `set`, `action`, `open_folder`, `dismiss_notice` actions), so the Luau load check stubs them as usual. The `plugin.<id>.*` names are built at run time and are deliberately outside the load check: the app's scripts never reference them, and a UI half is loaded only by the slot loader.

## UI slots

| Slot | Where | State |
|---|---|---|
| `settings` | the plugin's page in Settings, Plugins | generated from the schema (45); a UI half adding below it is reserved |
| `composer.buttons` | the composer's button row, left of GIF, sticker and emoji, in the main and thread composers | placed (47) |
| `panel.buttons` | the user panel, between the user area and mute | placed, empty until 56 |
| `rail.buttons` | the server rail, under Home | placed, empty until 54 |
| `user.decoration`, `member.decoration` | beside names in the timeline, member list and profiles | reserved for 49 |
| `message.menu` | items appended to the message context menu (`bridge::message_menu` and `MessageActions::menu_items`, which draws the timeline menu, the hover bar's More menu and search results) | reserved for 53 |
| `message.accessory` | under a message | reserved for 52 |

### Contributing to a slot

A plugin asks for each slot in its manifest and ships a `ui.luau` (or the file `ui = "..."` names):

```toml
permissions = ["hook:typing", "ui:composer.buttons"]
```

```lua
local settings = ui.shared("plugin.silent-typing.settings", {})

local toggle = ui.component(function(props: any)
	return ui.button {
		id = "silent_typing_" .. props.slot.area,
		on_click = function()
			ui.invoke("plugin.silent-typing.set", { key = "silent", value = not settings:get().silent })
		end,
		ui.svg { src = "keyboard_off.svg", w = 24, h = 24, tint = "$kyoko.danger" },
	}
end)

ui.contribute("composer.buttons", toggle {})
```

- The script runs once per load and returns nothing; `ui.contribute(slot, node)` adds to a slot. A contributed `ui.component` is built once per slot instance and gets the slot's context as `props.slot`: `{ area = "composer" }` or `{ area = "thread_composer" }` for `composer.buttons`, `{ area = "panel" }`, `{ area = "rail" }`. Build ids from it so two instances stay apart.
- Draw like the app: theme tokens as strings (`"$kyoko.interactive_normal"`, `"$kyoko.radius_sm"`, the names in `apps/desktop/ui/theme.luau`) and oxidcord's icon button sizes (a 32-point hit box and a 24-point icon in the composer). Images and modules come from the plugin's own folder only; the app's `icons/` and components are out of reach, so a plugin ships the icons it shows (Lucide's, as the app does).
- What it can reach: `ui.shared("plugin.<id>.settings", {})`, the plugin's current settings (read it; zgx lets a script write a shared value, but the host overwrites it and never reads it back); `ui.invoke("plugin.<id>.set", { key, value })`, which sets one of its own settings through `SettingSpec::accept`; `ui.invoke("plugin.<id>.action", { key, slot })`, which runs `on_action(key, { source = "slot", slot })` through `PluginHost::gesture` (`slot` is passed on only when it names a slot the plugin was granted). Both actions run only inside a user gesture (above). Its own `ui.state`, components and timers are its own.
- What it cannot reach, enforced by zgx: any other shared value or host action (the app's `composer.open` or `login.sign_out`, another plugin's `plugin.<other>.*`), the network (`ui.fetch`), the clipboard, file dialogs, terminals, web views and windows. Reaching for one is an error that leaves the plugin's UI half out.

### How the loader works

`apps/desktop/src/ui/plugin_slots.rs` (`PluginSlots`, owned by `PluginsView`). On every host change it takes `PluginHost::snapshot` and mounts each plugin that is `Running`, has a UI entry and is granted at least one placed slot: a `zgx_script::Plugin { name: id, entry, grants: [Capability::Named("plugin.<id>")] }` for `ScriptHost::set_plugins`, `plugin.<id>.settings` published and exposed (`expose_shared`) with that capability, and the two actions registered and exposed (`expose_action`) with it. It publishes `plugins.slots`, the granted plugin ids per placed slot. When the mounted set changes (a plugin starts, stops, is suspended, gains or loses a slot grant, or restarts, which bumps `PluginInfo::generation`), it calls `ScriptHost::reload`, which keeps every script's state. A changed setting only republishes `plugin.<id>.settings`.

- Bundled plugins live in the binary. When one starts, its UI files (everything but the manifest and the logic entry) are unpacked to `<data dir>/plugin-ui/<id>/`, rewritten only when they differ, and `PluginInfo::ui_entry` points there. A user or development plugin loads its `ui.luau` from its folder.
- The shell draws a slot with `components/plugin_slot.luau`: `plugin_slot { name = "composer.buttons", context = { area = area }, gap = 2 }` is one `ui.slot(name, { from = id, context })` per granted id, in id order, so a contribution to a slot the plugin was not granted is never shown. It takes no room while empty.
- A UI half that fails to parse, raises while it loads or reaches for something it was not given is left out by zgx; one that raises while its component builds draws nothing. Either way the shell and the other plugins load, the failure is logged with the plugin id (its text at debug level), and debug builds show zgx's error banner. The load check covers this (below).

### Adding a slot

A slot drawn by Luau, outside the timeline, takes a few lines:

1. Name it in `Slot` (`crates/plugins/src/permission.rs`) if it is new, and make `Slot::available` return `true` for it.
2. Add it to `PLACED` in `apps/desktop/src/ui/plugin_slots.rs`.
3. Place `plugin_slot { name = "<slot>", context = { ... } }` in the shell module, with the context a contribution needs.
4. Add a contribution to the probe plugin, the permission to `ALL_SLOTS` and its marker to `PROBE_MARKERS` in `apps/desktop/src/ui/scripts/plugins.rs`, so the load check builds it.

A slot inside the timeline cannot be a zgx contribution, because the Rust timeline sits above the whole Luau tree: `message.menu`, `message.accessory` and timeline decorations are drawn by Rust from data the logic half provides (a new hook returning plain data, cached per message revision, off the UI thread), and their clicks go through `PluginHost::gesture`. The member list is Luau and could take a zgx slot per row; profiles are Rust.

## ClearURLs

`plugins/clear-urls/`: `before_send` and `before_edit` clean every `http(s)` link outside code spans and code blocks, keeping the rest of the text, the link's order of parameters and its fragment. `rules.luau` has three lists: parameters removed everywhere (`utm_*`, `fbclid`, `gclid`, `msclkid`, `mc_eid`, …), per-site parameters (YouTube's and Spotify's `si`, X's `s` and `t`, Amazon's `ref` and `/ref=` path segments, …) and referral tags removed only while "Remove referral tags" is on (Amazon's `tag`, …). Nothing is fetched. The rule set is kyoko's own, written from the parameters' public documentation; the ClearURLs project's rules are LGPL-3.0 and are not used (`THIRD_PARTY_NOTICES.md`).

## SilentTyping

`plugins/silent-typing/` (`hook:typing`, `ui:composer.buttons`, ships disabled). Its `allow_typing` vetoes every typing indicator while the "Hide that I'm typing" setting (`silent`, on by default) is on, so no `POST /channels/{id}/typing` reaches Discord. Its UI half puts a keyboard button in the composer's button row: Lucide's `keyboard-off` in `danger` while silent ("Disable Silent Typing"), `keyboard` otherwise ("Enable Silent Typing"); a click sets `silent` through `plugin.silent-typing.set`. "Show the composer button" (`button`) hides it. The session asks the veto at most once per 9 s per channel, so after switching it off the next indicator goes out within 9 s of the last one it held back.

## Adding a bundled plugin

1. Create `plugins/<id>/` with `plugin.toml`, `logic.luau` and its modules. Ask for the fewest permissions that work.
2. Add its files to `crates/plugins/src/bundled.rs` (`include_str!` each file, UI half and icons included, into an `Embedded` list, and the list to `PACKAGES`).
3. Write its behaviour tests (below), document what it does here, and note any third-party data in `THIRD_PARTY_NOTICES.md`.
4. Bundled plugins ship disabled; the user enables and approves them in Settings, Plugins.

While writing it, run `cargo run -p kyoko -- --mock --plugins-dir plugins` and edit the files: the plugin reloads on save.

## Testing a plugin

Behaviour tests drive a session against an in-process mockcord with the plugin host as its hooks: `crates/plugins/tests/plugins/` (it reuses the session harness, `crates/session/tests/session/harness.rs`). `support::write_plugin` writes a test plugin into a scratch development folder, `support::start` starts the host and a session, `support::approve` approves what the manifest asks for. Observe behaviour through the session seam: the echoed message text, `State`, and mockcord's recorded traffic (`recording().requests_to(...)`). A handy trick for observing a plugin's internals is a `before_send` that appends what it saw to the message.

What is covered: ClearURLs on send and edit, SilentTyping (no typing request while on, typing again after the toggle or disabling, its UI half unpacked and remounted on restart), approval before running, re-approval when a manifest asks for more, hooks without their permission never running, `kyoko.*` refusing ungranted calls, read-only state copies, the typing veto, event scopes, the gesture rule and per-plugin rate limit, deadline overruns (including a `pcall`-wrapped loop) suspending a plugin without holding up sends, backoff and turning off, a plugin that does not load, storage and settings across reloads and a new host, and hot reload. The settings page itself is covered by `the_plugins_page_asks_before_a_plugin_is_enabled` in the Luau load check, and `cargo xtask headless-run --mock` runs ClearURLs, SilentTyping (no typing request reaches the mock) and a failing plugin against the mock.

UI halves are covered headless in `apps/desktop/src/ui/scripts/plugins.rs`, part of the load check: `the_interface_loads_and_builds_every_screen` mounts a probe plugin in every placed slot; `a_plugin_shows_only_in_the_slots_it_was_granted` (its settings value, its action naming only a granted slot); `a_broken_or_overreaching_plugin_interface_leaves_the_shell_building` (a syntax error, an error while loading, an error while building, and reaching for an app value, another plugin's value, the network and an app action, beside a working plugin); `the_silent_typing_button_switches_its_setting_and_unmounts_with_the_plugin` (the real `PluginsView` and host: a click without a gesture does nothing, a click with one switches `silent`, the button follows `button`, disabling unmounts it and enabling mounts it again). The `composer-silent-typing` capture clicks the real button in a window.
