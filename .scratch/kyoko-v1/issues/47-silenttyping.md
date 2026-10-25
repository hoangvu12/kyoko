# 47 — SilentTyping

**What to build:** Users can hide that they're typing, with a quick toggle in the composer.

**Blocked by:** 45 (Plugin runtime with ClearURLs)

**Status:** resolved

- [x] A before-typing hook can veto the typing signal
- [x] A composer button slot exists and hosts the toggle
- [x] Session tests assert that no typing traffic is sent while it's enabled

## Answer

Merged to main as `01111d6`. Docs: `docs/plugins.md` ("UI slots", "Contributing to a slot", "How the loader works", "Adding a slot", "SilentTyping"), `docs/ui.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md`.

- Plugin UI half: `apps/desktop/src/ui/plugin_slots.rs` (`PluginSlots`, owned by `PluginsView`) mounts a plugin's `ui.luau` while it runs, only into slots granted with `ui:<slot>`. A UI half sees only its own `plugin.<id>.settings`, `plugin.<id>.set` (change one of its own settings) and `plugin.<id>.action` (runs `on_action(key, { source = "slot", slot })` through `PluginHost::gesture`); both actions need a real click or key press within 1 s. zgx blocks app values and actions, other plugins' values, network, files, clipboard and windows. Remounts (state kept) on start, stop, suspension, disable and file-edit restarts (`PluginInfo::generation`); a settings change only updates the value. Bundled plugins' UI files are unpacked to `<data dir>/plugin-ui/<id>/` on start. A broken plugin UI is left out and logged with its id (debug builds also show zgx's error banner); the rest of the UI loads.
- Slots placed (`components/plugin_slot.luau`, no room while empty): `composer.buttons` (left of GIF, main and thread composers), `panel.buttons` (user panel, between user area and mute), `rail.buttons` (server rail, under Home). `Slot::available` covers these three. No Rust overlay layer.
- SilentTyping (`plugins/silent-typing/`, bundled, ships disabled, `hook:typing` + `ui:composer.buttons`): `allow_typing` vetoes while `silent` is on (default on); the button shows Lucide keyboard-off in red while hidden, keyboard otherwise, a click flips `silent`; setting `button` hides it. After switching off, the first typing post can wait up to 9 s because the session's throttle also starts on a vetoed signal (documented, session unchanged).
- Tests: `silent_typing_sends_no_typing_while_on_and_types_again_once_off` (zero `POST /channels/*/typing` while on), `a_bundled_interface_half_is_unpacked_and_remounted_on_each_start`, a headless `TestApp` test clicking the button with the real host (gesture rule, setting, unmount/remount), load-check tests `a_plugin_shows_only_in_the_slots_it_was_granted` and `a_broken_or_overreaching_plugin_interface_leaves_the_shell_building` (seven bad UIs: syntax, load, build errors, and reaching app values, other plugins, network, app actions), a probe plugin in all three slots. `headless-run` runs SilentTyping and fails if typing reaches the mock. Checks re-run by the orchestrator in lane C, all green.
- Parity (port 8493): composer 6.2%, new `composer-silent-typing` 6.2% (oxidcord side reuses `composer`; kyoko clicks the button at `capture::SILENT_TYPING_BUTTON`). `press_at` (profiles.rs) and `until_capture` (ui.rs) are `pub(super)`.
- For 54/56: the slots exist; ask for `ui:<slot>`, ship a `ui.luau` contributing a component, read `props.slot.area`, call `plugin.<id>.action { key, slot }`; `on_action` runs inside the gesture so `kyoko.actions.*` is allowed; feedback is a host-side toast. Adding a Luau slot: `Slot::available`, `PLACED` in `plugin_slots.rs`, `plugin_slot { name, context }` in the shell module, and a contribution plus `ALL_SLOTS`/`PROBE_MARKERS` entries in `ui/scripts/plugins.rs`. Plugins ship their own icons (app `icons/` are out of reach), use theme tokens as strings (`"$kyoko.danger"`), and list every file in `bundled.rs`. More data for a UI half: publish `plugin.<id>.<name>` and expose it with `Capability::Named("plugin.<id>")` in `PluginSlots::expose`, never app names.
- For 49/52: timeline decorations and message accessories can't be zgx slots (the Rust timeline is above the Luau tree); draw them in Rust from data a new logic hook returns (cached per message revision, off the UI thread), clicks through `PluginHost::gesture`. The member list is Luau and could take a slot per row; profiles are Rust. For 53: keep using `bridge::message_menu` and `MessageActions::menu_items`.
- Midnight: the slots will be restyled with the composer (M16), panel and rail (M03).
