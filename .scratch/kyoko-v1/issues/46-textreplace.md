# 46 — TextReplace

**What to build:** The user's replacement rules rewrite outgoing and edited text.

**Blocked by:** 45 (Plugin runtime with ClearURLs)

**Status:** resolved

- [x] A before-edit hook exists alongside before-send
- [x] Rules are configured in the plugin's settings
- [x] Session tests cover rewriting on send and on edit

## Answer

Merged to main as `dabd8a7..032c5bb`. Docs: `docs/plugins.md`, `docs/architecture.md`, `docs/testing.md`.

- Before-edit hook: `Hook::Edit` with its own permission `hook:edit` (it used to share `hook:send`). `before_edit({ channel_id, guild_id?, message_id, content })` returns a string or `nil`, chained in plugin id order like `before_send`, same 250 ms deadline and 4000-char / empty / unchanged rule; `SessionHooks::before_edit` returns `Option<Rewrite>`. A rewritten edit marks the message `State::altered_by` (`Internal::Edited`, `LocalEvent::EditAltered`); an unchanged edit while hooks are attached clears the mark. ClearURLs now asks for both permissions, so users who approved it see "Needs approval" once. The plugin API stays `api = 1` (no third-party plugins exist yet).
- Generic `list` setting type (replaces the `pairs` type 45's docs had reserved): `type = "list"` with `[[settings.fields]]` (boolean, number, string, select; up to 8), `item`, `max` (default 25, at most 100), `default`. Lenient: non-table rows dropped, bad cells take the field default, extra keys dropped, rows past `max` cut. Read with `kyoko.settings.get(key)` as an array of tables. The generated settings pane draws each row as a card with its fields and Move Up / Move Down / Remove, plus "Add <item>"; structural changes save at once, typing after 500 ms; reuses `widgets.button`, `widgets.switch`, `widgets.radios` and the input style (Midnight 19 restyles them). The editor keeps its own copy of rows while open. New setting types: `setting_json` in `src/ui/plugins.rs` and a branch in `field_row` in `settings/plugins.luau`, plus the fixture in `the_generated_plugin_settings_build_every_field_type`.
- TextReplace (`plugins/text-replace/`, bundled, ships disabled): ordered rules with Find, Replace with, "Only if the message includes" (plain text) and a "Luau pattern" switch. Rules run top to bottom, each on the previous result; plain rules are literal, pattern rules use Luau string patterns via `string.gsub` (not regex; `%1`/`%0`/`%%` documented). Each rule runs in its own `pcall`, so a bad pattern skips only that rule; a runaway pattern hits the plugin deadline and the message goes out as typed. Code spans and blocks are not skipped (the rules are the user's own, as in Vencord).
- Tests: five session-seam tests in `crates/plugins/tests/plugins/text_replace.rs` (exact POST `content` and PATCH body; order incl. reordering; captures; include condition on send and edit; broken rules; runaway pattern with a 100 ms deadline; plugin off), `sends_and_edits_are_rewritten_only_with_their_own_permission`, and the `TestApp` test `the_rules_editor_adds_edits_and_removes_rows`. `headless-run` sends and edits a typo through TextReplace.
- Plugin test load flake fixed (`032c5bb`): plugin test hosts (`support::config`, and the plugin UI lab in `ui/scripts/plugins.rs`) use a 15 s `load_deadline`; call deadlines unchanged, production `Limits::default()` unchanged (1 s). `xtask test` passed three times on a busy machine and once more for the orchestrator.
- Parity (port 8493): settings-plugins 4.3% (kyoko lists the three bundled plugins where oxidcord has none); `settings-text-replace` is a kyoko-only capture of the rules editor (`capture::TEXT_REPLACE_SETTINGS`), checked by eye by the orchestrator. Stale: the `settings-plugins` row in `docs/parity.md` says only ClearURLs; the plugin card's button row in `plugins.luau` uses `wrap = true`, which zgx ignores (the prop is `flex_wrap`).
- For 39: new strings in the list editor and TextReplace's manifest.
