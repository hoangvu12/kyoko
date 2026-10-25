# Desktop integration

Kyoko's Windows integration: notifications, the tray, a global hotkey, start at sign-in, the image clipboard, single instance and deep links. The rules live in `state` and `session`, the OS calls in `crates/platform`, and the glue in `apps/desktop/src/integration.rs`.

## Notifications

The session decides; the desktop shows.

`State::alert_for(message)` (`crates/state/src/notifications.rs`) returns an `AlertKind` or nothing:

- Nothing for your own messages, for authors you blocked, before READY, or in a muted guild, muted channel or muted parent category. Mutes with an `end_time` in the past have expired.
- A DM or group DM is `DirectMessage`, unless its channel override (in the guild-less settings entry) says `NoMessages`.
- In a guild, the level is the channel override (channel, then parent), else the guild entry's `message_notifications`, else the guild's `default_message_notifications`, else only mentions. `Inherit` and unknown levels fall through.
- `NoMessages`: nothing. Only mentions: `Mention` when the message mentions you. All messages: `Mention` when it does, `Message` otherwise.
- Mentions are direct user mentions, `@everyone`/`@here` unless `suppress_everyone`, and roles you hold unless `suppress_roles`.

The session (`crates/session/src/task/alerts.rs`) applies that to every live `MESSAGE_CREATE` and also drops the alert when:

- your status is Do Not Disturb (`Command::SetStatus`);
- the message is in the selected channel while viewing (`Command::SetViewing(true)`, the default, which the desktop keeps in step with window focus);
- the `allow_notification` hook returns false (run on a blocking worker; a failing hook allows);
- the burst limit is spent: `SessionConfig::alerts` (`AlertPacing`), three alerts per ten seconds by default.

Accepted alerts queue (at most 16) behind `Session::take_alerts()`, and `Changes::alerts` wakes the UI. The desktop formats them with `bridge::toast_text` (author, `#channel, Guild` or group name; mentions, channels, roles and custom emoji made readable; 200 characters) and posts a toast on a worker thread.

Settings: `notifications.enabled`, `notifications.show_content` (off: "Mentioned you" or "Sent a message"), `notifications.sounds` (silenced by streamer mode's `disable_sounds`).

Toasts are posted under the AppUserModelID `io.github.hoangvu12.Kyoko`, also in `--mock` runs. At startup the desktop writes `HKCU\Software\Classes\AppUserModelId\io.github.hoangvu12.Kyoko` (`DisplayName` Kyoko, `IconUri` a copy of the icon in the cache directory), which an unpackaged app needs for toasts to show. Clicking a toast while kyoko runs shows the window and opens the channel. Windows' global notification switch (`PushNotifications\ToastEnabled`) and Focus Assist still apply.

## Window, tray and hotkey

- The tray icon is created on zgx's UI thread inside the run closure. Its menu is Show Kyoko, Hide Kyoko and Quit Kyoko. A left click toggles: hide when kyoko is in front, otherwise show and focus.
- The icon shows a red badge when any channel or DM has mentions, a white dot when something unmuted is unread, and the plain icon otherwise. The tooltip says "Kyoko: N mentions" or "Kyoko: unread messages". It is recomputed from `State::attention()` at most every 750 ms after read state, guild, DM or settings changes.
- Closing the window hides it to the tray when `window.minimize_to_tray` is on (the default, as in oxidcord) and a tray exists; otherwise it quits. Quit from the tray always quits.
- `keybinds.toggle_window` (default unset; for example `ctrl+alt+shift+KeyK`, global-hotkey syntax) toggles the window from anywhere. It is registered at startup and again whenever the Keybinds page changes it; a binding Windows refuses shows as a note under the field.
- Every 500 ms the desktop checks whether the foreground window belongs to kyoko and sends `Command::SetViewing` when that changes, and `SetViewing(false)` when the window is hidden. This also pauses automatic read acks while kyoko is in the background.
- Windows may refuse to bring the window forward (focus-stealing rules) when it is shown while another app is active. Showing from a hidden state, a second launch or a toast normally succeeds.

## Start at sign-in

`startup.open_on_login` (default off) writes or removes the `kyoko` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` as `"<exe>" --background`. `--background` starts kyoko hidden in the tray. The setting is applied at every start of a real run, and of a mock run with `--data-dir`.

## Single instance and deep links

zgx owns single instance. A real run uses the application id `io.github.hoangvu12.Kyoko`; a mock run with `--data-dir` uses `io.github.hoangvu12.Kyoko.Mock`; a mock run on scratch data has none, so parallel test runs never meet. Before starting a session, `platform::instance::is_running` probes zgx's pipe (`\\.\pipe\<id>.<session>`). If it is held, the launch runs zgx's hand-over (its links go to the running kyoko, which takes the foreground) and exits without connecting. If the probe misses, zgx still hands over inside `Application::run`.

The running kyoko shows and focuses its window for every later launch. URLs are parsed by `bridge::parse_deep_link`:

- `discord://-/channels/<guild>/<channel>[/<message>]`, `discord:///channels/…`, `discord://channels/…`, the same under `kyoko://`, and `https://discord.com/channels/…` (also `ptb.`, `canary.` and `discordapp.com`); `@me` is a DM.
- Other `discord://` and `kyoko://` links only bring the window forward. Anything else is ignored, including the `file:` URLs zgx makes of path arguments.
- A guild channel link selects the guild and opens the channel. DM links and message ids only focus the window until the DM list and jump-to-message exist.

The first launch's own arguments arrive as zgx's first `OpenUrls`. They open their link without forcing the window up, so `--background` stays hidden.

### URL protocol

Registering `discord://` is opt-in: `integration.discord_links` (default off) writes `HKCU\Software\Classes\discord` pointing at this executable. Turning it off removes the key only if it still points at kyoko, so the official client's handler is never removed. It is never registered silently, because the official client registers the same scheme. `kyoko://` is not registered at run time; the installer (ticket 58) should register it for the current user, with the same Start menu AppUserModelID.

## Image clipboard

The host action `media.copy_image` takes an image name (`media/<hex>`, with or without `host:`), reads the decoded frame from `media::Gallery::pixels` and writes it to the clipboard as an image on a worker thread. An image that is not loaded is logged and skipped. Context menus (28, 32) call it.

## Mock traffic

`kyoko --mock --pings N` sends the user a DM or a mention every N seconds, alternating, to exercise notifications and the tray badge.
