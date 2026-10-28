# Packaging

Everything here is driven by `.github/workflows/release.yml` on a `v*` tag. Each script reads `VERSION` (no leading `v`) and writes to `dist/` using the asset naming from `docs/release.md`.

| Path | What it is |
| --- | --- |
| `windows/installer.nsi` | NSIS per-user installer into `%LOCALAPPDATA%\Programs\Kyoko`, start menu shortcut, uninstaller, `Uninstall` registry keys under HKCU, silent `/S`. |
| `windows/kyoko.rc` | Resource template. `apps/desktop/build.rs` fills in the icon path and version and compiles it with `embed-resource`, so the exe carries `assets/icon.ico` and version info. |
| `windows/plugins/x86-unicode/WinShell.dll` | Optional, not committed. When present the installer sets the AppUserModelID `io.github.hoangvu12.Kyoko` on the shortcut through `WinShell::SetLnkAUMI`, which Windows needs to attribute toast notifications. Drop the Unicode build of the NSIS WinShell plugin there before building the installer. |

Assets live in `assets/`: `icon.png` (256 px: the README and toasts), `icon_32.png` (the tray) and `icon.ico` (16, 24, 32, 48, 64 and 256 px). The logo is a square crop of fan art credited in `THIRD_PARTY_NOTICES.md`: 48 px and up show the face with the goggles and hair clip, 24 and 32 px a closer crop, 16 px the eyes and mouth only.

## What CI does

1. `verify` checks the tag matches the `kyoko` crate version.
2. `windows` builds the MSVC release binary, runs `makensis` and zips the portable build.
3. `publish` collects the artifacts, runs `cargo xtask release notes`, and creates the GitHub release.
