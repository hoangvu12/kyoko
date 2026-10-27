# 56 — GameActivityToggle

**What to build:** A button next to the user panel toggles whether game activity is shared, ready for when kyoko reports activities.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 30 (User panel, status and profiles)

**Status:** resolved

- [x] A panel button slot exists and hosts the toggle
- [x] The toggle's state persists and presence updates honour it
- [x] The plugin ships disabled until kyoko reports activities

## Answer

Merged to main as `e4af1ec..42a571d`. Docs: `docs/plugins.md` (GameActivityToggle, `action:activity`), `docs/gateway.md` (the presence rule), `docs/parity.md`, `THIRD_PARTY_NOTICES.md`.

- Plugin `plugins/game-activity-toggle/` (bundled, ships disabled; `PACKAGES` now 15) asks for `action:activity` and `ui:panel.buttons`. `ui.luau` puts a controller button in `panel.buttons` drawn like mute, deafen and gear (32-point `$kyoko.button_size` hit box, 20-point icon in `text_4`, `text_3` on the hover background): Lucide `gamepad` with tooltip "Disable Game Activity" while sharing, a crossed-out controller in `$kyoko.danger` with "Enable Game Activity" while hidden. A click calls `plugin.game-activity-toggle.action { key = "toggle", slot = "panel.buttons" }`.
- State: `on_action` calls the new `kyoko.actions.share_activity(on?)` (needs `action:activity`, gesture only; `nil` flips and returns the new value), which sends the existing `Command::SetActivitySharing`, the same `show_current_game` settings-proto write as Settings, Activity Privacy (M19), so both always agree and a refusal rolls back with the existing toast. The window publishes `plugin.<id>.activity` from `State` to plugins granted `action:activity` (`PluginSlots::share_activity`, from `plugins_apply`). New `PluginRequest::ActivitySharing` shows "Game activity is hidden" / "Game activity is shared again" (exhaustive matches need an arm).
- Presence: kyoko sends no game activities today (IDENTIFY's presence activities are typed as an empty array; op 3 carries only the custom status). `publish_presence` now drops every activity except the custom status while `show_current_game` is off; future activities must be built inside `publish_presence` (and IDENTIFY given the same rule if it ever carries them).
- Tests: plugin seam `crates/plugins/tests/plugins/game_activity_toggle.rs` (ships disabled with exactly its two permissions; a gesture click flips `show_current_game` with one settings-proto write per click reaching mockcord, false then true; other sources do nothing; one toast request per click); session `with_activity_sharing_off_presence_carries_nothing_but_the_custom_status` in `status.rs` (IDENTIFY and every op 3; guards the future, can't fail today); load check `the_game_activity_button_sits_in_the_panel_follows_sharing_and_asks_for_its_toggle`; `headless-run` clicks off and on and checks both writes (`activity_toggled`).
- Checks re-run by the orchestrator in lane A on `cd7de71`: fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny all exit 0; luau0-src `0.22.0+luau740`. Kyoko-only capture `user-panel-game-activity` (port 8494, `KYOKO_CAPTURE_PLUGINS=game-activity-toggle`, button at `capture::GAME_ACTIVITY_BUTTON`): red crossed-out controller beside mute and the "Game activity is hidden" toast, checked by eye.
- Deviations: Lucide has no `gamepad-off`; `gamepad_off.svg` is kyoko's own variant (Lucide's gamepad opened around Lucide's slash). The button sits 12 points from mute, not 4, because of the existing panel slot's gaps (Midnight 03 layout left alone).
