# Interface

Kyoko's interface is a 1:1 port of oxidcord 0.2.0-beta.1, written in Luau on zgx. The Rust host in `apps/desktop/src/` owns the window, the theme and everything that touches the session. The Luau scripts in `apps/desktop/ui/` lay out and draw. This page is the pattern every UI ticket follows. `docs/parity.md` says how a screen is compared with oxidcord.

## Where things live

```
apps/desktop/src/
  ui.rs            the window: Link (session to Luau), host actions, shared values, the Rust root layout
  theme.rs         oxidcord's theme tokens, theme files, zgx theme data
  fonts.rs         installed font families, picking the first of a font list
  frame.rs         the native window: maximized state, the exit waker
apps/desktop/ui/
  .luaurc          strict mode; aliases @components and @shell
  app.luau         the entry: requires the shell and returns its root
  theme.luau       every theme token by name, as zgx token strings
  state.luau       every shared value the host publishes, typed, in one place
  actions.luau     every host action and function the scripts call
  components/      reusable pieces with no screen of their own (icon, icon_button)
  shell/           one module per region of the window
  icons/           oxidcord's icon set, one SVG per icon name
```

A screen or component is one module that returns a function building its node. Use `ui.component` only when the piece needs per-instance state (`ui.use_state`), as `components/icon_button.luau` does for hover. Modules require what they use: `require("../theme")`, `require("../state")`, `require("../actions")`, `require("@components/icon")`. In debug builds the host reloads the scripts when a file changes, so Luau work needs no rebuild. Rebuild Rust only for host changes.

## Shell regions

`shell/frame.luau` lays the window out exactly as oxidcord's `Shell::render` does. Sizes come from the theme (`theme.size.*`), which holds oxidcord's values.

| Region | Module | Size | Filled by |
|---|---|---|---|
| Title bar: mark, app name, drag region, minimize/maximize/close | `title_bar.luau` | full width, `title_bar_height` 22 | 20 (done) |
| Server rail | `server_rail.luau` | `server_rail_width` 72 | 22 |
| Channel or DM sidebar, with the guild header | `channel_sidebar.luau` | `channel_sidebar_width` 240, top-left corner `radius_md`; hidden below 900 wide | 23, 24 |
| User panel at the bottom of the sidebar | `user_panel.luau` | `user_panel_height` 52 | 30 |
| Top bar: channel icon, name, topic, threads, notifications, pins, members, search, inbox, help | `top_bar.luau` | `top_bar_height` 48 with a 1-point bottom border | 20 (done); 31 search, 33 threads and pins, 35 notifications |
| Timeline | `timeline.luau` | grows; reports its box as the timeline slot | 25 replaces it with the Rust view, 26 renders messages |
| Composer | `composer.luau` | 44-point field, 16 sides, 24 below | 27 |
| Member list | `member_list.luau` | `member_list_width` 240, `background_secondary`; shown at 1200 wide and up, toggled by the members button | 29 |

Overlays (menus, popovers, dialogs, toasts, the lightbox, settings) are added beside the shell's root by the tickets that build them (28, 30–38).

### The timeline slot

zgx cannot place a Rust view inside a Luau tree, so the Rust root layout in `ui.rs` is an overlay: the Luau view fills the window, and a Rust view is absolutely positioned above it at the box the Luau timeline reports. `shell/timeline.luau` calls `actions.report_slot("timeline", element)` from `on_resize`. The host function `frame.slot` turns that into the `Slot` signal `timeline_slot` follows. Today the Rust slot is empty and lets the pointer through, and the spike's Luau message list shows beneath it. Ticket 25 puts the Rust timeline view in the slot and removes the Luau list.

## Theme

`apps/desktop/src/theme.rs` holds oxidcord's tokens in one place: `COLORS` and `SIZES` with oxidcord's dark defaults, the `light` and `amoled` overrides, and the UI and code font lists. `Palette` is a theme in oxidcord's file format (`docs/themes.md`). `Catalog::load` reads the built-in themes, every `themes/<id>/theme.json` beside the source (or `KYOKO_THEMES_DIR`), and the user's `<data dir>/themes/*.json`, all through the same parser. `Catalog::install` turns each into a zgx theme and applies `settings.theme` (`dark` by default; `system` follows the system's light or dark). Ticket 57 builds the catalog and the colour import on this; ticket 37 switches themes from settings with `ui.theme.set(name)`.

Every token is the zgx app token `kyoko.<name>`: `kyoko.background_primary`, `kyoko.radius_md`, `kyoko.ui_font`. zgx's standard tokens are mapped from them too (`background`, `text.primary`, `accent`, `font.sans`, `font.mono`, scrollbars, hover washes), so zgx's own widgets, scrollbars and menus match the theme.

Luau reads tokens only through `theme.luau`, never as literal colours:

```lua
local theme = require("../theme")
ui.row { h = theme.size.top_bar_height, bg = theme.color.background_primary,
	ui.text { "general", text_size = theme.size.font_md, weight = theme.weight.semibold, color = theme.color.header_primary } }
```

`theme.color.*` and `theme.size.*` are token strings, so a theme switch restyles without running the script. A property that only takes a number (such as `rounded_tl`) uses `theme.number(theme.size.radius_md)`. A new token goes into `COLORS` or `SIZES` and into `theme.luau` together. The only literal colour allowed is a fully transparent `#00000000` to clear a control's own background.

## Fonts

Kyoko bundles no fonts, as oxidcord bundles none. `fonts::installed` reads the installed families from the Windows font registry and `fonts::pick` takes the first installed family of the theme's list: `gg sans`, `Noto Sans`, `Segoe UI`, `Helvetica Neue`, `Arial`, `sans-serif` for the interface and `Consolas`, `Menlo`, `monospace` for code. On a stock Windows machine that is Segoe UI and Consolas, the same faces oxidcord renders. The shell root sets `font = theme.font.ui`.

## Icons

`icons/<name>.svg` holds every icon oxidcord's `IconKind` names, in snake case: `hash`, `volume`, `megaphone`, `forum`, `thread`, `members`, `pin`, `bell`, `inbox`, `help`, `search`, `settings`, `mic`, `headphones`, `window_close` and so on (122 files; `ls apps/desktop/ui/icons`). Show one by name:

```lua
local icon = require("@components/icon")
icon { name = "hash", size = 24, color = theme.color.text_muted }
```

Without `color` an icon takes `currentColor`, the text colour it inherits, so it follows its button's `color` and `hover`. `components/icon_button.luau` is oxidcord's `IconButton`: a 28-point hit box, a 20-point icon in `interactive_normal`, `interactive_hover` on hover and `interactive_active` when `active` returns true. Sources and licences are in `THIRD_PARTY_NOTICES.md`.

## Shared values and host actions

Names are `<area>.<name>` in snake case, with the area the region or service that owns them. Plugins (45) will get `plugin.<id>.<name>`.

| Kind | Names |
|---|---|
| Shared values (Rust publishes, Luau reads) | `session.status`, `rail.guilds`, `sidebar.channels`, `sidebar.guild_id`, `sidebar.guild_name`, `timeline.channel_id`, `timeline.channel_name`, `timeline.channel_topic`, `timeline.messages`, `user.avatar`, `user.avatar_ready`, `window.maximized` |
| Actions (`ui.action`, fire and forget) | `rail.select_guild`, `sidebar.select_channel`, `window.minimize`, `window.maximize`, `window.close`, `media.copy_image` |
| Functions (`ui.call_sync`) | `media.visible`, `media.hover`, `media.reduced_motion`, `frame.slot` |

Rust registers them in `Link::install` (and `Integration::install` for desktop integration). Luau declares each shared value once in `state.luau` and wraps each action in `actions.luau`, so a rename touches one Luau file. State only the interface cares about stays in Luau: `ui.state` in the module that owns it, or in `state.luau` (`state.shell.members_visible`) when several modules share it. Never publish the whole store. Publish small projections from `bridge` and let zgx reconcile them.

## Window frame

The window is frameless (`Titlebar::Hidden`): Windows keeps its shadow, rounded corners and resize edges, and the title bar is kyoko's own, as oxidcord's is. The drag region is `window_drag = true`: a double click maximizes and a right click opens the system menu. The buttons call `window.minimize`, `window.maximize` and `window.close`. `frame.rs` finds the native window to read whether it is maximized (`window.maximized` swaps the maximize and restore icons). Windows 11 snap layouts on the maximize button need zgx's caption hit testing, which only `Titlebar::Inset` has, so they are missing for now.

## Adding a screen

1. Add the module under `shell/` (or `components/` if it is reusable), styled only from `theme.luau`, with icons by name.
2. Publish what it needs from Rust as `<area>.<name>` shared values, declare them in `state.luau`, and add its actions to `actions.luau`.
3. Add its capture steps (`docs/parity.md`) and compare `pair.png` with oxidcord.
