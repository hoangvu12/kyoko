# Plugins

Kyoko runs Luau plugins modelled on Vencord's hook surface. The contract is owned by the core and versioned: this page is **plugin API v1**. Tickets 46 to 57 build on it, and a change to anything here bumps the version.

The runtime is `crates/plugins` (`PluginHost`). It has no UI dependency and builds headless. It plugs into the session only through the optional `SessionHooks` interface (`docs/architecture.md`), so the session knows nothing about plugins, and a plugin failure never stops it.

## The two halves

- **Logic half** (`logic.luau`): data hooks, events, storage and actions. Each enabled plugin gets its own sandboxed Luau VM on a worker thread of its own, with a call deadline on every hook, so a per-message hook never runs on the UI thread or blocks the session for long. This half is built.
- **UI half** (`ui.luau`): a zgx plugin contributing to named UI slots, reaching nothing of the app's but the `plugin.<id>.*` values and actions the app exposes to it. It runs on the UI thread in its own sandboxed zgx VM, mounted while the plugin runs. The placed slots are `composer.buttons`, `panel.buttons` and `rail.buttons`; the settings pane is generated from the manifest and needs no UI code, and so are `message.menu` items. See "UI slots".

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
permissions = ["hook:send", "hook:edit"]
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
| `[[menu]]` | | context-menu items, below ("Menu items") |

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
| `list` | `fields` (a `[[settings.fields]]` table per column), `item` ("Item"), `max` (25, at most 100), `default` (a list of rows) | a list of rows, each a table with one value per field | a card per row with its fields, Move Up, Move Down and Remove, and an "Add <item>" button |

The host validates every value (`SettingSpec::accept`); an invalid value changes nothing. Values live in `<data dir>/plugins.json` with the plugin's enabled flag and approved permissions.

A `list` is generic: any plugin with rows of settings can use it. Each field is a setting of its own (`key`, `type`, `label`, its extra fields and `default`) of type `boolean`, `number`, `string` or `select`; at most 8 fields, and a field of another type is left out. Validation is lenient: a row that is not a table is dropped, a missing or invalid cell takes the field's default, extra keys are dropped and rows past `max` are cut, so one bad row never loses the others. A plugin reads the list with `kyoko.settings.get(key)` as an array of tables, in the order the user set.

```toml
[[settings]]
key = "rules"
type = "list"
label = "Rules"
item = "Rule"
max = 50

[[settings.fields]]
key = "find"
type = "string"
label = "Find"

[[settings.fields]]
key = "pattern"
type = "boolean"
label = "Luau pattern"
```

The editor keeps its own copy of the rows while the page is open: adding, moving or removing a row and flipping a switch save the whole list at once; typing saves it 500 ms after typing stops. A new row starts with every field's default.

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
| `hook:send` | `before_send`: read and rewrite the text you send, DMs included | yes |
| `hook:edit` | `before_edit`: read and rewrite the text of your edits, DMs included | yes |
| `hook:typing` | `allow_typing`: veto your typing indicator | yes |
| `hook:notify` | `allow_notification`: read messages that would notify you, and silence them | yes |
| `hook:display_name` | `format_name`: change how people's names show in the timeline, the member list and profiles | yes |
| `hook:timestamp` | `format_relative`: change the text of relative times (`<t:...:R>`) | yes |
| `hook:emoji_rank` | `rank_emoji`: reorder the emoji autocomplete, seeing the query and your favourite emoji | yes |
| `hook:message_render` | message render access to kept deleted and edited versions | reserved for 52 |
| `action:mark_read` | `kyoko.actions.mark_all_read`, `mark_guild_read` | yes |
| `action:upload_expression` | `kyoko.actions.clone_expression`: offer to copy the emoji or sticker the user chose from a menu into a server | yes |
| `action:activity` | the activity-sharing toggle | reserved for 56 |
| `ui:composer.buttons`, `ui:panel.buttons`, `ui:rail.buttons` | show its UI half in that slot | yes |
| `ui:message.menu` | add its `[[menu]]` items to the message menu and the expression menus | yes |
| `ui:settings`, `ui:user.decoration`, `ui:member.decoration`, `ui:message.accessory` | contribute to that UI slot | reserved (see UI slots) |
| `network:<host>` | contact that host | reserved; v1 has no network API at all |

Reserved permissions parse, show in the prompt with "Not used yet", and grant nothing until their ticket adds the call path (`Permission::available`). The prompt marks sensitive ones (`hook:send`, `hook:edit`, `hook:notify`, `read:messages`, `read:dms`, actions, network). Granted permissions are `declared ∩ approved`.

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
	format_name = function(person) return nil end,
	format_relative = function(time) return nil end,
	rank_emoji = function(request) return nil end,
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
| `before_edit(message)` | `hook:edit` | before an edit is sent | `{ channel_id, guild_id?, message_id, content }` | as `before_send` | 250 ms |
| `allow_typing(channel)` | `hook:typing` | before each typing indicator (at most one per 9 s per channel) | `{ channel_id, guild_id? }` | `false` vetoes; anything else allows | 100 ms |
| `allow_notification(message)` | `hook:notify` | after kyoko's own notification rules and the burst limit, for each OS notification and its sound about to be shown (never for in-window toasts) | `{ id, channel_id, guild_id?, author_id, author_name, content }` | `false` silences; anything else allows | 100 ms |
| `format_name(person)` | `hook:display_name` | off the UI thread, once per person and inputs, then cached (below) | `{ user_id, guild_id?, username, global_name?, nick?, name, extra? }`; `name` is what kyoko shows (the server nickname, else the display name, else the username) | a string to show instead of `name`, or `{ name?, extra? }`, where `extra` is shown after it, dimmed; `nil` for no change | 50 ms |
| `format_relative(time)` | `hook:timestamp` | off the UI thread, once per distinct span, then cached | `{ seconds, past, unit, value, text }`: whole seconds between now and the time, whether it is in the past, the largest unit that fits (`second`, `minute`, `hour`, `day`, `month` of 30 days, `year` of 365 days), the exact count of that unit (`2.96`), and kyoko's own text (`3 days ago`) | a string to show instead, or `nil` | 50 ms |
| `rank_emoji(request)` | `hook:emoji_rank` | after typing pauses on a `:` query (below) | `{ query, guild_id?, emojis }` | an array of keys to put first, or `nil` | 100 ms |
| `on_event(event)` | the event's read scope | after each listed gateway event is applied to state | see Events | nothing | 200 ms |
| `on_action(key, context)` | none | when the user clicks the plugin's `action` setting, one of its slot buttons or one of its menu items | the key, and `{ source = "settings" }`, `{ source = "slot", slot = "composer.buttons" }` or `{ source = "menu", slot = "message.menu", target?, id?, name?, animated?, format?, channel_id?, message_id? }` ("Menu items") | nothing | 500 ms |
| `on_settings_changed()` | none | after the user changes one of its settings | nothing | nothing | 200 ms |

- Text hooks chain in plugin id order: each sees the previous plugin's output. A replacement that is empty, longer than 4000 characters or equal to the input counts as no change. A rewritten send or edit is recorded as altered by the last plugin that changed it (`State::altered_by`); an edit no plugin changed clears the mark. There is no "cancel" in v1: the spec asks for rewriting only.
- Vetoes stop at the first plugin that returns `false`.
- Display hooks chain too: each sees the previous plugin's `name` and `extra` (or `text`). A name or text that is empty or longer than 100 characters counts as no change; an empty or too long `extra` is dropped. See "Display hooks" below for when they run.
- A failing or overrunning hook counts as a strike (below) and the session goes on with the unmodified value; a veto hook that fails allows.
- `allow_notification` is asked only about alerts the burst limit would show: the session reserves the alert's place first and a veto gives it back, so a plugin that remembers what it allowed is never told about an alert the user did not get. Calls for separate messages may run in either order. Silencing a notification changes nothing else: unread counts, mention badges and the tray badge stay as they are. `guild_id` is filled from state when the gateway leaves it out.

### Emoji ranking

`rank_emoji` reorders the composer's `:` autocomplete. It never runs on the UI thread and never holds up typing:

- The autocomplete shows kyoko's own order at once. Each new query (two letters or more after `:`) sends `Command::RankEmoji` with the candidates; the session waits until no new query has come for 100 ms (`RANK_DELAY` in `crates/session/src/task/emoji_rank.rs`), runs at most one ranking at a time, and keeps only the latest query waiting. The result is stored in `State` for that server, query and favourites generation and replaces kyoko's order when it arrives; a stale result is ignored.
- Plugin calls: one per query typing pauses on, per plugin with the permission. Typing `:smile` quickly is one call; at one keystroke per 150 ms it is one per letter from the second. Moving the selection, new messages or member updates never call it, and the same query is asked once per favourites generation (a second call only when favourites arrive or change while it is shown). With no running plugin holding `hook:emoji_rank`, nothing is called at all.
- `request.query` is the text after `:` in lowercase; `request.guild_id` is the composer's server. `request.emojis` are kyoko's first 100 matches in its own order, plus every later match that is a favourite. Each is `{ key, name, animated, id?, guild_id?, emoji?, favorite? }`: `key` is a custom emoji's id or a Unicode emoji's shortcode, `emoji` a Unicode emoji's characters, `favorite` its 1-based place in your favourite emoji (`docs/rest.md`, "Favourite emoji").
- The return value is a list of keys: those emoji go first in that order, unknown and repeated keys are ignored, and every emoji not listed follows in the order it had. Anything that is not a table changes nothing. Ranking hooks chain in plugin id order, each seeing the previous order.
- A ranking that fails or overruns its 100 ms counts a strike and leaves the order as it was, kyoko's own if no plugin ranked; the autocomplete is never emptied or held.

### Events

`on_event` gets `{ type = ..., ... }`, never a raw gateway payload:

| `type` | Fields | Scope |
|---|---|---|
| `ready` | `user_id`, after each live READY (a new connection or another account; not a resume) | none |
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
| `kyoko.state.channel(id)` | `read:guilds` for server channels, `read:dms` for DMs and group DMs (either one to call it) | `{ id, guild_id?, name, type, parent_id? }` or `nil`; `nil` too for a channel the plugin may not read |
| `kyoko.state.read_state(channel_id)` | `read:read_state` | `{ channel_id, last_message_id?, mention_count }` or `nil` |
| `kyoko.actions.mark_all_read()`, `mark_guild_read(guild_id)` | `action:mark_read` | nothing; raises if refused |
| `kyoko.actions.clone_expression()` | `action:upload_expression` | nothing; raises if refused. Only inside `on_action` for a menu item chosen on a custom emoji or an image sticker: kyoko opens its clone dialog for exactly that expression |
| `require("./name")` | | a module file of the same package, loaded once |

State queries read the session's `State` snapshot under its read lock and copy what they return, so shared Discord data is read-only to plugins. There is no API to write app state, fake client features, change the identity, reach REST or the gateway, or bypass a permission check. `print` is removed; the Luau standard library is the sandboxed one (`string`, `table`, `math`, `utf8`, `bit32`, `buffer`, `coroutine`, `os.time`/`clock`/`date`).

## Display hooks

`format_name` and `format_relative` change what the window draws, and the window draws every frame, so they never run on the UI thread or per frame. `bridge::Display` (`crates/bridge/src/display.rs`) stands between the two:

- The window only looks into its cache. A hit is used at once; a miss shows kyoko's own text, and the question goes to the `display-hooks` thread, which asks `PluginHost` (and through it each plugin's worker, with its 50 ms deadline). When a batch of answers is in, the desktop gets one wake (`Formatting::Names` or `Formatting::RelativeTimes`) and redraws only what changed: the timeline re-projects its rows (an author whose name changed is a `Delta.updated` row; for relative times, the rows whose text has a `<t:...:R>`), the thread pane does the same, the member list republishes its window and an open profile is recomputed.
- Names are cached per `Person`: the user, the server, and the username, display name and nickname themselves, so a rename or a new nickname is a new question and nothing else is. Relative times are cached per signed span in whole seconds. The caches hold 4096 names and 1024 spans, and start over when full.
- The cache is dropped whenever `formatters(kind)` changes: a formatting plugin is enabled, disabled, suspended, restarted or reloaded, or one of its settings changes (`PluginHost::set_setting` bumps the plugin's revision). While the new answers come in, the old ones stay on screen, so a settings change does not flash kyoko's names; when no plugin formats any more, kyoko's own text is back at once.
- The desktop connects the two with `PluginHost::on_change(display.refresher())`. Projections ask only while some plugin formats (`Display::formats`), so with no such plugin a row costs one atomic check.
- A failing, overrunning or suspended plugin falls back to kyoko's own text for that name or span, which is cached too, so a broken plugin is asked once per person, not once per frame; three strikes suspend it as for every hook. A slow plugin holds up only the `display-hooks` thread, never a row: the row shows kyoko's text until the answer arrives.
- What is formatted: the author of every message in the timeline and the thread pane (not system lines, reply previews, mentions or search results), each member row, and the name on the profile popout and full profile. Relative times are those of `<t:...:R>` in message bodies. Kyoko's own relative text rounds to the nearest unit ("2 days and 23 hours ago" is "3 days ago"), as Discord's does.

A full mock session (`cargo xtask headless-run --mock`, 150 messages of one channel with ShowMeYourName and DontRoundMyTimestamps on) makes 43 to 53 display calls (runs differ by when members' nicknames arrive, each a new question): one per distinct author and nickname seen, and one for the relative time it checks; re-projecting the channel 100 times more makes none.

## Actions and the gesture rule

A Discord action runs only inside a user gesture:

1. The gesture starts in the app, never in a plugin: a click or key press on the plugin's button in kyoko's interface. The window records each pointer-down and key-down; `plugins.action` and a UI half's `plugin.<id>.action` and `plugin.<id>.set` are refused unless one happened in the last second, and consume it, so a script calling them in a loop or on a timer gets nothing.
2. The host calls `on_action(key, context)`; only during that call is `kyoko.actions.*` allowed, and only once per gesture. From any other hook or event it raises `kyoko: ... was refused: actions only run from your click or command`.
3. Each plugin may run at most 5 actions per 10 seconds (`Limits::actions`, `action_window`), across gestures and restarts.
4. An allowed action is a session `Command` (`MarkAllRead`, `MarkGuildRead`) sent through the same `Commander` as the user's own clicks, so it goes through the same locks, pacing and REST safety layer (`docs/locks.md`, `docs/rest.md`). Plugins never reach around them. `clone_expression` sends no command at all: it asks the window (`PluginHost::on_request`, `PluginRequest::Clone`) to open kyoko's clone dialog, and the upload is the user's own click on a server there, a `CreateEmoji` or `CreateSticker` through the same `Commander` as server settings' uploads. A plugin names no expression: it can only clone the one the user right-clicked, which the host keeps for the length of the call.

`PluginHost::gesture(id, key, context)` is the entry point; `PluginHost::menu_gesture(id, key, slot, target, context)` is the same for a menu item (it checks the item is one the plugin declared and was granted, and adds `source`, `slot` and the target's fields to the context). The desktop calls either only after the activation check above. The spec's other gesture, a command, will call the same entry point once plugins can register commands.

## Deadlines, failures and the disable policy

| Limit | Default (`Limits`) |
|---|---|
| send and edit hooks | 250 ms |
| typing and notification vetoes | 100 ms |
| `format_name`, `format_relative` (`name_deadline`, `time_deadline`) | 50 ms each |
| emoji ranking | 100 ms |
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
| `message.menu` | items appended to the message context menu (`bridge::append_plugin_entries` after `bridge::message_menu`, drawn by `MessageActions` for the timeline, the thread pane, the hover bar's More menu and search results), and the menu of a custom emoji or sticker cell in the expression pickers | placed (53), from the manifest's `[[menu]]` |
| `message.accessory` | under a message | reserved for 52 |

### Menu items

`message.menu` is drawn by Rust (the timeline sits above the Luau tree), so a plugin declares its items in the manifest instead of a UI half, and asks for `ui:message.menu`:

```toml
permissions = ["ui:message.menu", "action:upload_expression"]

[[menu]]
slot = "message.menu"
key = "clone_emoji"
label = "Clone Emoji"
icon = "copy_plus.svg"
when = ["custom_emoji", "can_create_expressions"]
```

- `key` (as a setting key), `label` (at most 64 characters) and `slot` (only `message.menu`) are required; an item without them is left out. At most 8 items per plugin, keys unique.
- `icon` is an SVG file of the plugin's own folder (at most 64 KiB), drawn at 12 points in the item's text colour (`currentColor`); without one the item keeps an empty icon box.
- `when` lists conditions that must all hold. Kyoko evaluates them from its own data each time a menu opens (`bridge::MenuFacts`), never by calling the plugin: `custom_emoji` (the pointer was on a custom emoji: inline in a message, a reaction, or a picker cell), `image_sticker` (on a PNG, APNG or GIF sticker, in a message or the picker), `can_create_expressions` (the user has Create Expressions or Manage Expressions in at least one server). An unknown condition never holds, so its item never shows. Without `when` an item shows in every message menu.
- Items follow kyoko's own, after a separator, in plugin id order. A picker cell's menu shows only items whose conditions match its expression.
- Choosing one runs `on_action(key, context)` through `PluginHost::menu_gesture`, a user gesture: `source = "menu"`, `slot`, and for an expression `target` (`"custom_emoji"` or `"sticker"`), `id`, `name` and `animated` (`"true"`/`"false"`) or `format` (Discord's sticker format number), plus `channel_id` and `message_id` in a message's menu.
- Where the pointer was: inline emoji, reaction chips and stickers in the timeline mark themselves on a secondary press (`timeline::actions::aimed`) and the row's own press opens the menu, so the menu knows its target without a hit test of its own.

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

`plugins/clear-urls/` (`hook:send`, `hook:edit`): `before_send` and `before_edit` clean every `http(s)` link outside code spans and code blocks, keeping the rest of the text, the link's order of parameters and its fragment. `rules.luau` has three lists: parameters removed everywhere (`utm_*`, `fbclid`, `gclid`, `msclkid`, `mc_eid`, …), per-site parameters (YouTube's and Spotify's `si`, X's `s` and `t`, Amazon's `ref` and `/ref=` path segments, …) and referral tags removed only while "Remove referral tags" is on (Amazon's `tag`, …). Nothing is fetched. The rule set is kyoko's own, written from the parameters' public documentation; the ClearURLs project's rules are LGPL-3.0 and are not used (`THIRD_PARTY_NOTICES.md`).

## TextReplace

`plugins/text-replace/` (`hook:send`, `hook:edit`, ships disabled). The user's rules, a `list` setting (`rules`, at most 50), rewrite every message they send and every edit. Each rule has Find, Replace with, Only if the message includes, and Luau pattern.

- Rules run from top to bottom, once each, every rule on the text the rules above it left; Move Up and Move Down change the order. A rule replaces every occurrence of Find.
- A plain rule (Luau pattern off) matches Find literally and inserts Replace with as it is.
- A pattern rule uses Luau's string patterns through `string.gsub`, not regular expressions: `%d` a digit, `%a` a letter, `.` any character, `*`, `+`, `-` and `?` repeat, `^` and `$` anchor, `(...)` captures; in Replace with, `%1` to `%9` are captures, `%0` the whole match and `%%` a percent sign. Plugins get no regex engine, and patterns keep it that way.
- "Only if the message includes" is plain text, checked against the text as the rules above left it; empty means always.
- An empty Find does nothing. A rule that raises (a malformed pattern such as `[a`, a `%` the replacement cannot use) is skipped and the rest still run; it is not a strike, because each rule runs in its own `pcall`. A pattern that runs away (`a*a*a*a*b` on a long line) is stopped by the plugin deadline: Luau's matcher checks the interrupt at every step, the overrun survives `pcall`, the hook counts a strike and the message goes out exactly as typed. Sending is never held longer than the caller's limit.
- Code spans and code blocks are not skipped: the rules are the user's own, so whether one touches code is theirs to decide by what it matches. Vencord's TextReplace, the behaviour reference, also applies to the whole message.
- A rewrite that leaves the message empty or longer than 4000 characters counts as no change, as for every text hook.

## SilentTyping

`plugins/silent-typing/` (`hook:typing`, `ui:composer.buttons`, ships disabled). Its `allow_typing` vetoes every typing indicator while the "Hide that I'm typing" setting (`silent`, on by default) is on, so no `POST /channels/{id}/typing` reaches Discord. Its UI half puts a keyboard button in the composer's button row: Lucide's `keyboard-off` in `danger` while silent ("Disable Silent Typing"), `keyboard` otherwise ("Enable Silent Typing"); a click sets `silent` through `plugin.silent-typing.set`. "Show the composer button" (`button`) hides it. The session asks the veto at most once per 9 s per channel, so after switching it off the next indicator goes out within 9 s of the last one it held back.

## OnePingPerDM

`plugins/one-ping-per-dm/` (`hook:notify`, `read:dms`, `read:read_state`, ships disabled). After one notification in a conversation, its `allow_notification` silences the next ones there, both the Windows notification and its sound, until the conversation has been read past the message that notified: you opened it while the window had focus, marked it read, or read it on another device. Read state is the source of truth; the plugin only keeps, in memory, the id of the last message that notified per channel, and clears it when a different account signs in (`ready`). Unread counts, mention badges and the tray badge are untouched.

- "Limit" (`channels`): all direct messages (default), one-on-one DMs only, or group DMs only. Telling them apart reads the channel's type with `kyoko.state.channel`; a channel kyoko does not know counts as neither.
- "Also limit server channels" (`servers`, off): server notifications (mentions, or every message where a server is set to notify for all) ping once per channel until read, too.
- A notification held back by the burst limit is never seen by the plugin, so it does not count as the one ping.

Vencord's OnePingPerDM is the behaviour reference (its choice of which DMs count); no code is shared. Vencord also has switches to let mentions and `@everyone` inside DMs through; they are left out, since a mention in a DM is already a DM notification.

## ShowMeYourName

`plugins/show-me-your-name/` (`hook:display_name`, ships disabled). Its `format_name` puts the username beside the name kyoko shows (the server nickname, else the display name), dimmed, or shows it instead, in the timeline, the member list and profiles.

- "Show" (`mode`): "Nickname, then username" (`nick-user`, the default: `Gecko quiet_gecko`), "Username, then nickname" (`user-nick`: `quiet_gecko Gecko`) or "Username only" (`user`).
- "Hide the username when it matches" (`hide_same`, on): one name when the shown name is the username, ignoring case.

Vencord's ShowMeYourName is the behaviour reference for the modes; no code is shared. Its reply and friend-nickname switches are left out, since kyoko formats message authors, member rows and profiles only.

## DontRoundMyTimestamps

`plugins/dont-round-my-timestamps/` (`hook:timestamp`, ships disabled). Its `format_relative` rounds relative times down instead of to the nearest unit: `math.floor(time.value)` of the unit that fits, so 2 days and 23 hours ago reads "2 days ago" where kyoko says "3 days ago", and 59 minutes 59 seconds reads "59 minutes ago" rather than "1 hour ago". Vencord's DontRoundMyTimestamps is the behaviour reference.

## FavoriteEmojiFirst

`plugins/favorite-emoji-first/` (`hook:emoji_rank`, ships disabled). Its `rank_emoji` puts every suggested emoji that is one of your favourites first, in the order you favourited them, and leaves the rest in kyoko's order. Favourites are Discord's own, read from the frecency settings the web client uses, so favourites set in the official client count, and favouriting on another device reorders the next suggestions. It changes only the composer's `:` suggestions; the expression picker's search keeps kyoko's order until the picker gains a Favorites section (Midnight 08). Vencord's FavoriteEmojiFirst is the behaviour reference; no code is shared.

## ExpressionCloner

`plugins/expression-cloner/` (`ui:message.menu`, `action:upload_expression`, ships disabled). Right-clicking a custom emoji (inline in a message, a reaction, or a cell of the emoji picker) offers "Clone Emoji", and a PNG, APNG or GIF sticker (in a message or the sticker picker) "Clone Sticker", while the user may add expressions to some server; a Unicode emoji and a Lottie sticker never offer it. Its `on_action` calls `kyoko.actions.clone_expression()`, which opens kyoko's clone dialog (`community/clone.rs`, a community dialog drawn above the timeline):

- The preview, the name (the source's, cleaned to Discord's rules; an emoji keeps letters, digits and underscores) and the servers where the user has Create Expressions or Manage Expressions (`bridge::clone_targets`), each with its free slots of that kind. A server at its limit for that kind (still and animated emoji count apart) is dimmed with "No free emoji slots (50 of 50)" and cannot be picked. Enter in the name takes the first open server.
- One click on a server is one clone: the dialog closes, the source image is fetched through the media pipeline (`MediaService::bytes`: the gallery's fetcher, disk cache and host allowlist; `bridge::clone_source`: an emoji's `.png` or `.gif` from the CDN with `quality=lossless`, a sticker's `.png`, `?passthrough=true` for APNG, or `.gif`), and kyoko sends `CreateEmoji` or `CreateSticker` (a sticker's tags and description come from the original when kyoko knows it, else its name). Another clone waits until the first has arrived or failed. The session checks and paces it like every server write (`docs/rest.md`); "Added :name: to Server" shows once the gateway brings it, and a refusal is the usual error toast.
- Nothing is batch-cloned or repeated.

Vencord's ExpressionCloner (GPL) is the behaviour reference only; no code is shared.

## Adding a bundled plugin

1. Create `plugins/<id>/` with `plugin.toml`, `logic.luau` and its modules. Ask for the fewest permissions that work.
2. Add its files to `crates/plugins/src/bundled.rs` (`include_str!` each file, UI half and icons included, into an `Embedded` list, and the list to `PACKAGES`).
3. Write its behaviour tests (below), document what it does here, and note any third-party data in `THIRD_PARTY_NOTICES.md`.
4. Bundled plugins ship disabled; the user enables and approves them in Settings, Plugins.

While writing it, run `cargo run -p kyoko -- --mock --plugins-dir plugins` and edit the files: the plugin reloads on save.

## Testing a plugin

Behaviour tests drive a session against an in-process mockcord with the plugin host as its hooks: `crates/plugins/tests/plugins/` (it reuses the session harness, `crates/session/tests/session/harness.rs`). `support::write_plugin` writes a test plugin into a scratch development folder, `support::start` starts the host and a session, `support::approve` approves what the manifest asks for. Observe behaviour through the session seam: the echoed message text, `State`, and mockcord's recorded traffic (`recording().requests_to(...)`). A handy trick for observing a plugin's internals is a `before_send` that appends what it saw to the message.

What is covered: ClearURLs on send and edit, TextReplace (the exact bodies sent and edited, rule order, the include condition, broken and runaway rules not breaking the send, nothing while off), `before_edit` only with `hook:edit`, SilentTyping (no typing request while on, typing again after the toggle or disabling, its UI half unpacked and remounted on restart), FavoriteEmojiFirst (`favorite_emoji_first.rs`: favourites first in their favourite order and after favourites change, the hook only with its permission, failing and endless rankings leaving kyoko's order beside a working one, nothing while off), OnePingPerDM (`one_ping_per_dm.rs`: three DMs notify once and stay unread, reading or viewing the DM lets the next one notify, the group/one-on-one choice, server mentions untouched unless asked, nothing while off, a failing or overrunning filter letting the notification through), the `ready` event, approval before running, re-approval when a manifest asks for more, hooks without their permission never running, `kyoko.*` refusing ungranted calls, read-only state copies, the typing veto, event scopes, the gesture rule and per-plugin rate limit, deadline overruns (including a `pcall`-wrapped loop) suspending a plugin without holding up sends, backoff and turning off, a plugin that does not load, storage and settings across reloads and a new host, and hot reload. The settings page itself is covered by `the_plugins_page_asks_before_a_plugin_is_enabled` and `the_generated_plugin_settings_build_every_field_type` (every setting type, a list included) in the Luau load check, `the_rules_editor_adds_edits_and_removes_rows` drives TextReplace's rules editor against the real host, and `cargo xtask headless-run --mock` runs ClearURLs, TextReplace (a sent and an edited typo), SilentTyping (no typing request reaches the mock), OnePingPerDM (three DMs, one notification; another once read), ShowMeYourName and DontRoundMyTimestamps (every author of the loaded channel shown through `bridge::Display`, no calls on re-projection, a relative time rounded down), FavoriteEmojiFirst (the mock's favourite ranked first) and a failing plugin against the mock. The session side of ranking (one read of the frecency proto, updates, the debounce and a failing hook) is covered in `crates/session/tests/session/favourites.rs`, and the real composer with the real plugin in the bridge test `favourites::the_emoji_autocomplete_lists_favourites_first_once_favorite_emoji_first_runs`.

ExpressionCloner: `crates/plugins/tests/plugins/expression_cloner.rs` (it ships disabled; its items join the menu only once approved; a menu click asks for exactly the clicked expression, nothing for a Lottie sticker, no target, a settings click or an undeclared key; `clone_expression` is refused outside a menu gesture and past the per-plugin limit, and without `action:upload_expression`); `crates/bridge/tests/bridge/cloner.rs` with the real host and a session against mockcord (the items show only for custom emoji and image stickers and only while some server takes expressions; a full server listed with why; the animated source fetched through `MediaService` and uploaded as a GIF, a sticker too); the session seam in `crates/session/tests/session/uploads.rs` (the exact emoji body and sticker form, permission, slots, names, types and sizes refused before REST, a Discord refusal as a rejection); and `timeline::clone_tests` in the desktop (right-click on the rendered emoji, Clone Emoji, the dialog's server, the emoji arrives). `headless-run` clones an emoji through the plugin (`clone_ok`).

Display hooks: `display.rs` covers each hook only with its own permission, ShowMeYourName's modes and its hide setting, DontRoundMyTimestamps' rounding, a failing and an endless formatter falling back and being suspended, and the formatter list changing with settings, restarts and disabling. `crates/bridge/tests/bridge/display.rs` drives the real host through `bridge::Display` against mockcord: the timeline's authors, the member list and a profile card show the username, redrawing asks no plugin again, a mode change and turning the plugin off redraw the rows, a stuck plugin never holds up a projection, and only rows with a relative time redraw for DontRoundMyTimestamps. The load check builds a member row with a dimmed username.

UI halves are covered headless in `apps/desktop/src/ui/scripts/plugins.rs`, part of the load check: `the_interface_loads_and_builds_every_screen` mounts a probe plugin in every placed slot; `a_plugin_shows_only_in_the_slots_it_was_granted` (its settings value, its action naming only a granted slot); `a_broken_or_overreaching_plugin_interface_leaves_the_shell_building` (a syntax error, an error while loading, an error while building, and reaching for an app value, another plugin's value, the network and an app action, beside a working plugin); `the_silent_typing_button_switches_its_setting_and_unmounts_with_the_plugin` (the real `PluginsView` and host: a click without a gesture does nothing, a click with one switches `silent`, the button follows `button`, disabling unmounts it and enabling mounts it again). The `composer-silent-typing` capture clicks the real button in a window.
