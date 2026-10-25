# oxidcord reference build

`oxidcord-ref.ps1` recreates the oxidcord build that kyoko's UI is compared against. See `docs/parity.md` for the capture workflow.

```
tools/parity/oxidcord-ref.ps1                  # clone next to this checkout, patch, debug build
tools/parity/oxidcord-ref.ps1 -Force           # replace an existing reference checkout
tools/parity/oxidcord-ref.ps1 -Path D:\ox      # somewhere else; then pass --oxidcord to the capture
tools/parity/oxidcord-ref.ps1 -Release         # release build in target\release
tools/parity/oxidcord-ref.ps1 -Export          # write the checkout's changes back to the patch
```

The script clones `https://github.com/Master0fFate/oxidcord`, checks out `b852253` (release 0.2.0-beta.1), applies `oxidcord-ref.patch` and runs `cargo build -p oxidcord`. Set `CARGO_BUILD_JOBS` beforehand to limit the build. A cold debug build took about 40 minutes with other builds running and uses about 7 GB of disk.

oxidcord is MIT OR Apache-2.0. The patch is a modification of its source and stays under those licences.

## What the patch changes

The patch only makes oxidcord drivable for captures. Normal runs behave as before.

- `apps/desktop/src/main.rs`: `--mock-server <url>` points `--mock` at a mockcord that is already running, using its REST v10, gateway, token and media origin. `--capture-screen <name>` and `--capture-ready <path>` pass a capture request to the UI. Login screens start without a token.
- `crates/ui/src/shell/capture.rs`: the capture driver. It waits for the default selection, runs the steps for the named screen through the shell's own methods (and synthetic mouse events for hover and context menus), waits 2 seconds, then writes the ready file. Unknown screens and failed steps exit with code 4.
- `crates/ui/src/app.rs`: `LaunchConfig::capture`, which starts the driver, and accessors for the current shell or login screen.
- `crates/ui/Cargo.toml`: `anyhow` for the driver's errors.
- Small visibility changes and helpers the driver needs: `custom_status` made public, `UserPanel::toggle_menu` and `LoginScreen::acknowledge` made crate-visible, `FriendsPage::show_tab`, `ServerRail::open_all_folders` and `SettingsWindow::show_named`.

## Changing the patch

1. Edit the reference checkout, usually `crates/ui/src/shell/capture.rs`, and rebuild it with `cargo build -p oxidcord`.
2. Run `tools/parity/oxidcord-ref.ps1 -Export` to write every change since `b852253` back to `oxidcord-ref.patch`.
3. Check that `tools/parity/oxidcord-ref.ps1 -Force` still applies and builds it, then commit the patch together with the matching rows in `docs/parity.md`.
