# 40 — Tray, notifications and desktop integration

**What to build:** kyoko integrates with Windows:

- a tray icon
- OS notifications that follow Discord's notification settings
- global hotkeys
- autostart
- image clipboard
- single instance
- deep links

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** resolved

- [x] The tray shows unread state, and offers show, hide and quit
- [x] OS notifications fire for mentions and DMs, following per-server and per-channel settings, under kyoko's app user model id (toasts posted without error; no banner could be seen because Windows notifications are off on the dev machine)
- [x] Global hotkeys and autostart work
- [x] Images can be copied to the clipboard
- [x] A second launch focuses the running window and passes any deep link to it

## Answer

Merged to main as `4f0e876..fe3579b`. Docs: `docs/desktop.md`.

- Rules in `state`: `State::alert_for(message)` → DM / mention / message / nothing (not own, blocked, muted guild/channel/category with unexpired mutes; DMs unless overridden to nothing; guild: channel override → guild setting → guild default → only mentions; `@everyone`/roles respect suppress). Session filters (`task/alerts.rs`): Do Not Disturb, the open channel while focused, the `allow_notification` hook (a failing hook allows), burst limit 3 per 10 s (`SessionConfig::alerts`). Delivered via `Session::take_alerts()`; the desktop formats with `bridge::toast_text` and posts under `io.github.hoangvu12.Kyoko`.
- Tray: Show / Hide / Quit, left click toggles, red badge for mentions, white dot for unread, tooltip "Kyoko: N mentions". `window.minimize_to_tray` (default on): close hides to tray. Every 500 ms the desktop sends `SetViewing` from foreground state (25 must not add a second sender).
- Settings: `notifications.{enabled, show_content, sounds}`, `window.minimize_to_tray`, `startup.open_on_login` (off; Run key `"<exe>" --background`), `keybinds.toggle_window` (unset by default, registered once at startup), `integration.discord_links` (opt-in `discord://` registration; turning it off removes the key only if it points at kyoko).
- Single instance via zgx `application_id` (`.Kyoko`, `.Kyoko.Mock` with `--data-dir`, none for a scratch mock run); `platform::instance::is_running` probes zgx's pipe so a second launch hands over before starting a session. Links: `discord://`, `kyoko://`, `https://discord.com/channels/...` (`bridge` parser). `platform::single_instance` removed. Capture mode skips all integration.
- Host action `copy_image(name)` copies a decoded gallery image to the clipboard.
- Mock flags `--pings N` and an accepted positional `[LINK]...`. Every run registers `HKCU\Software\Classes\AppUserModelId\io.github.hoangvu12.Kyoko` (needed for unpackaged toasts).
- Tests: 6 in `tests/session/alerts.rs`, 4 deep-link tests in `crates/bridge/tests/bridge/deep_links.rs`. Verified by running on mock: tray menu, badge, Quit (219 ms), close-to-tray, hotkey, `--background`, second launch hand-over (85 ms) with a guild-channel deep link, autostart Run key, link-handler registration (tested on a throwaway scheme), image copy.
- For 37: expose the settings above; autostart/link handler apply at startup (`integration::apply_startup_settings`); hotkey re-registration needs a small new API. For 51: implement `SessionHooks::allow_notification` (runs after kyoko's rules, before the burst limit). For 54: the tray could add "Mark all read". For 58: Start-menu shortcut with the AUMID, register `kyoko://` for the user, uninstall removes the Run value and the AUMID key. DM links/toasts only focus the window until a DM list exists (24).
