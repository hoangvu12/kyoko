# 20 — Theme, fonts, icons and app frame

**What to build:** kyoko's window frame looks exactly like oxidcord's: theme colours, fonts, icons, title bar and top bar.

**Blocked by:** 19 (Parity capture tooling)

**Status:** resolved

- [x] oxidcord's theme tokens (colours, UI font, code font) are ported to zgx theme data
- [x] oxidcord's icons are available to Luau
- [x] The title bar and top bar match oxidcord in the parity captures, apart from text-rendering differences

## Comments

From the spike (04): about one window close in three leaves the process running after the window is gone (seen under mock with no channel open too); the cause inside zgx is not found. The app frame should find and fix it (or report it upstream to zgx); 04 made the scratch folder get deleted as soon as the session stops.

## Answer

Merged to main as `064aa0a`. Conventions for every later UI ticket: `docs/ui.md`.

- Theme: `apps/desktop/src/theme.rs` holds oxidcord's 45 colours, 89 sizes, UI and code font lists and the `light`/`amoled` overrides as zgx app tokens `kyoko.<name>`, with zgx standard tokens mapped from them. `themes/midnight`, `themes/amoled-violet` and `<data>/themes/*.json` load through the same parser (`theme::Catalog`, `Palette::from_json`, for 57). `settings.theme` (`system` follows the OS); switch with `ui.theme.set(name)` (`dark`, `light`, `amoled`, file names). Luau reads tokens only via `ui/theme.luau`.
- Fonts: none bundled (oxidcord bundles none); the same preference lists as oxidcord resolved from the Windows font registry (Segoe UI and Consolas here; gg sans only if installed).
- Icons: 122 SVGs in `apps/desktop/ui/icons/<name>.svg` named after oxidcord's `IconKind`, from `gpui-kit-assets` 0.8.0 (118 Lucide 1.43.0 ISC/Feather MIT, 4 GPUI Kit Apache-2.0), in `THIRD_PARTY_NOTICES.md`. Shown with `components/icon` and `components/icon_button`.
- Frame: frameless window (`Titlebar::Hidden`) with oxidcord's 22 px title bar and 48 px top bar (channel icon, name, topic, threads, bell, pins, members, search, inbox, help); regions sized like oxidcord with its 900/1200 px breakpoints. Region modules in `ui/shell/`, assembled by `frame.luau`: `title_bar`, `server_rail` (22), `channel_sidebar` (23, 24; rounded top-left; guild header inside), `user_panel` (30), `top_bar` (buttons wired by 31, 33, 35), `timeline` (25, 26), `composer` (27), `member_list` (29). Login screens (21) go beside the shell root in `frame.luau` and reuse `title_bar`.
- Timeline slot: the Rust root overlays an empty click-through Rust view on the box the Luau timeline reports via `frame.slot` (`timeline_slot` in `ui.rs`, follows `on_resize`); 25 fills it and deletes the Luau list.
- Luau conventions: one module per region/component; `state.luau` declares every shared value, `actions.luau` wraps every host action/function; names `<area>.<name>` (plugins `plugin.<id>.*`); no literal colours. 40's `copy_image` is now `media.copy_image`.
- Parity: frame lines up pixel for pixel (title bar 0–21, top bar 22–69, member list at x=1040). Title bar 1.47% and top bar 5.25% differing (app name, ClearType vs greyscale text, slightly thinner icon strokes). Whole window 11.0% (content owned by 22–30).
- Exit hang: zgx calls `event_loop.exit()` from `about_to_wait` without changing control flow, so winit's Windows loop can sleep forever in `MsgWaitForMultipleObjectsEx` (bare zgx window reproduces it). Workaround `frame::wake_until_loop_ends`, started from `on_closed`, posts a no-op every 50 ms until `run` returns (max 10 s): 0 hangs in 44 cycles vs 2 in 20 without. Repro and suggested upstream fix (`ControlFlow::Poll` on exit) in `docs/spike.md`.
- Snap layouts on the maximize button need zgx caption hit testing (only for `Titlebar::Inset`); noted in `docs/ui.md`.
