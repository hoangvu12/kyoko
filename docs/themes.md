# Themes

A theme changes colours, and a kyoko `theme.json` or a Midnight-style `.css` can change sizes too, and a `theme.json` fonts. There is no code and nothing runs, so a theme cannot do anything except change how Kyoko looks.

Settings, Themes (in the Kyoko section, beside Plugins) lists three kinds:

- **Built In**: Midnight (the default), Dark, Light, AMOLED and the first party themes in the repository's `themes/` folder. Apply makes one the base theme. Appearance's Midnight, Dark, Light, AMOLED and System choices write the same setting (`theme`).
- **Local Themes**: the files in your themes folder, each with a switch.
- **Online Themes**: links you add, one per line.

Enabled local themes and every online theme apply on top of the base theme, live: local themes first, in the order you turned them on, then online themes in the order of their lines. A later theme wins where two set the same colour. Turning a theme off (or deleting its file, or removing its line) brings back the colours underneath at once.

Each card shows the theme's name, version, author and description, a strip of six of its colours as they look on your base theme, and any error.

## Midnight

Midnight is refact0r's Midnight theme (Vencord flavour 2.1.1, MIT, [refact0r/midnight-discord](https://github.com/refact0r/midnight-discord)) and kyoko's default: warm greys, a rose accent, Inter for the interface (bundled, `docs/ui.md`), `gg mono`, Source Code Pro or Consolas for code. Its values are the flavour's own (`MIDNIGHT` in `crates/bridge/src/themes/tokens.rs`), and every other colour follows from them, as Midnight's stylesheet derives Discord's colours from its variables. `docs/design/midnight.md` is the design it implements; the Midnight restyle tickets move each screen onto its tokens (`docs/ui.md`, "Midnight tokens").

The bundled theme that was called Midnight before (deep blue, by Master0fFate) is now **Deep Blue**, `themes/deep-blue`. Saved settings are read as they are: `"theme": "midnight"` is this Midnight.

## Local themes

1. Open Settings, Themes, and use Open Themes Folder.
2. Drop a `.css` file (a Vencord or BetterDiscord theme) or a `.json` file (a kyoko theme) in it. A folder holding a `theme.json`, and optionally a `theme.toml`, works too.
3. It shows up under Local Themes within a second. Turn it on with its switch.

The folder is `<data dir>/themes`; see the README for where that is on your system. `--data-dir` moves it. Kyoko checks the folder once a second, so editing, adding or deleting a file applies without a restart. A file is identified by its file name (`nord.css`), a theme folder by the folder name. Files over 512 KB, files that are not text and malformed JSON show their error on the card and are never applied; the other themes keep working.

## Online themes

Paste direct links to theme files into the box, one per line, and press Save Links, as Vencord's Online Themes tab works:

```
https://raw.githubusercontent.com/someone/ocean/main/ocean.theme.css
@light https://someone.github.io/paper/paper.css
@dark https://someone.github.io/ink/theme.json
```

- Blank lines and repeated lines are dropped, and at most 64 links are kept.
- A line that starts with `@light` or `@dark` applies only while the base theme is light or dark.
- A link must be a direct link to the file (a raw GitHub link, GitHub Pages, a CDN), not a web page. A link ending in `.json`, or a file that is a JSON object, is read as a kyoko theme; anything else as CSS.

The session downloads the links, never the interface: over HTTPS only (an `http:` link, a redirect to `http:` or a link with a user name is refused), with no Discord token, cookies or super properties, at most 512 KB each, giving up after 20 seconds, at most 3 redirects. The links download at startup once the session is signed in, when you save the list and when you press Refresh. Each good download is cached in `<cache dir>/theme-links/`, and when a download fails the card shows the error and keeps using the last good copy, offline too. Before you sign in, the cached copies apply. One failing link never stops the others.

## Importing a Vencord or BetterDiscord theme

Vencord and BetterDiscord themes are CSS written for Discord's web page, so most of a theme (layouts, fonts, images, animations, anything aimed at Discord's class names) cannot apply to Kyoko. Kyoko keeps only the colours, and the sizes of a Midnight-style file:

- It reads the custom properties (`--name: value`, Discord's variable names and Midnight's) declared on `:root`, `html`, `body`, `#app-mount`, `.theme-dark`, `.theme-light`, `.visual-refresh` and similar whole-app selectors, including inside `:is()`, `:where()` and `@media` blocks. Properties scoped to a part of the page (`[class*="sidebar"] { ... }`) and every ordinary declaration are ignored.
- Properties declared under `.theme-light` (or `prefers-color-scheme: light`) apply only on a light base, those under `.theme-dark` only on a dark one.
- `var(--x)` and `var(--x, fallback)` are resolved inside the file, so a theme built on its own variables (`--accent: hsl(var(--hue) 60% 40%)`) imports correctly.
- Colours may be hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`), `rgb()`/`rgba()`, `hsl()`/`hsla()` (comma or space syntax, `/ alpha`, angle units) and common colour names. `!important` is dropped. `color-mix()` is read (below). Anything else (`calc()`, gradients, images, an unresolved variable, a cycle) is skipped, never fatal.
- The details come from the first `/** ... */` comment, one `@tag value` per line, the BetterDiscord format: `@name`, `@author`, `@description`, `@version`, `@source`, `@website`. The name defaults to the file name.

Discord's variable names (Vencord, BetterDiscord) land on the Midnight tokens the restyled screens read, so a theme that sets only Discord's variables recolours the Midnight shell, and everything built on those tokens follows (`text_0` from `bg_4`, `border_light` from `hover`, `mention` and `accent_new` from `accent_2`, `reply` from `text_3`, `offline` from `text_4`):

| Midnight token | Discord variables, first found wins |
| --- | --- |
| `bg_4` (panels) | `--background-primary`, `--background-base-low`, `--background-secondary`, `--background-base-lower`, `--background-secondary-alt` |
| `bg_3` (the window between the panels) | `--background-tertiary`, `--background-base-lowest` |
| `bg_2` (dark buttons) | `--button-secondary-background` |
| `bg_1` (dark buttons pressed) | `--button-secondary-background-hover` |
| `hover` | `--background-modifier-hover` |
| `active` | `--background-modifier-selected` |
| `active_2` | `--background-modifier-active` |
| `message_hover` | `--background-message-hover` |
| `border` | `--background-modifier-accent` |
| `text_0` | `--white-500` |
| `text_1` | `--interactive-active` |
| `text_2` | `--header-primary`, `--text-strong` |
| `text_3` | `--text-normal`, `--text-default`, `--interactive-hover` |
| `text_4` | `--interactive-normal`, `--channels-default`, `--text-muted`, `--header-secondary`, `--text-subtle` |
| `text_5` | `--interactive-muted` |
| `accent_1` (links) | `--text-link` |
| `accent_2`, `accent_3` | `--brand-experiment`, `--brand-500`, `--brand-experiment-500` |
| `accent_4` | `--brand-experiment-560`, `--brand-560` |
| `online`, `idle`, `dnd`, `offline` | `--status-online`, `--status-idle`, `--status-dnd`, `--status-offline` |

A Midnight token's own variable (`--bg-4`) wins over the Discord names on it. Each Discord variable lands on one Midnight token, except Discord's brand colour, which is both Midnight's small accents (`accent_2`) and its accent buttons (`accent_3`). Midnight's panels are all one surface, so a theme's primary and secondary backgrounds both feed `bg_4` (primary first), and its tertiary background becomes the window around them. Discord's danger, warning and positive colours stay on oxidcord's `danger`, `warning` and `success`: Midnight's red, yellow and green scales are base palettes a Discord theme has no names for.

The same Discord variables also keep landing on oxidcord's tokens of the same meaning, which the screens not yet restyled read. Each of these is built on the Midnight token the variable also lands on (`background_secondary` on `bg_4`, `text_muted` on `text_4`), so a theme that sets one variable gives both the same colour, and a theme that sets several keeps them apart on the older screens (a theme's secondary background stays on `background_secondary` while `bg_4` takes its primary one). The test `a_discord_variable_lands_on_one_midnight_token_and_what_is_built_on_it` holds the mapping to this.

| Kyoko token | Discord variables |
| --- | --- |
| `background_primary` | `--background-primary`, `--background-base-low` |
| `background_secondary` | `--background-secondary`, `--background-base-lower` |
| `background_tertiary` | `--background-tertiary`, `--background-base-lowest` |
| `background_floating` | `--background-floating`, `--background-surface-highest` |
| `user_panel_background` | `--background-secondary-alt` |
| `modifier_hover` | `--background-modifier-hover` |
| `modifier_selected` | `--background-modifier-selected` |
| `modifier_active` | `--background-modifier-active` |
| `text_normal` | `--text-normal`, `--text-default` |
| `text_muted` | `--text-muted` |
| `header_primary` | `--header-primary`, `--text-strong` |
| `header_secondary` | `--header-secondary`, `--text-subtle` |
| `interactive_normal` | `--interactive-normal` |
| `interactive_hover` | `--interactive-hover` |
| `interactive_active` | `--interactive-active` |
| `interactive_muted` | `--interactive-muted` |
| `text_on_brand` | `--white-500` |
| `brand` | `--brand-experiment`, `--brand-500`, `--brand-experiment-500` |
| `brand_hover` | `--brand-experiment-560`, `--brand-560` |
| `link` | `--text-link` |
| `mention_background` | `--mention-background` |
| `mention_text` | `--mention-foreground` |
| `status_online` | `--status-online`, `--status-positive` |
| `status_idle` | `--status-idle`, `--status-warning` |
| `status_dnd` | `--status-dnd`, `--status-danger` |
| `status_offline` | `--status-offline` |
| `danger` | `--status-danger`, `--text-danger` |
| `danger_hover` | `--button-danger-background-hover` |
| `success` | `--status-positive`, `--text-positive` |
| `warning` | `--status-warning`, `--text-warning` |
| `scrollbar_thumb` | `--scrollbar-auto-thumb`, `--scrollbar-thin-thumb` |
| `scrollbar_track` | `--scrollbar-auto-track` |
| `divider` | `--background-modifier-accent` |
| `input_background` | `--input-background`, `--channeltextarea-background` |
| `button_secondary` | `--button-secondary-background` |
| `button_secondary_hover` | `--button-secondary-background-hover` |
| `spoiler` | `--spoiler-hidden-background` |
| `focus_ring` | `--focus-primary` |
| `overlay` | `--background-scrim` |

A few of Discord's semantic names land on Midnight washes: `--background-feedback-critical`, `-positive`, `-warning`, `-info`, `--border-feedback-warning`, `--input-background-error-default`, `--reaction-background-reacted-default`, `--user-panel-button-active-background`, `--overlay-button-background` and `--switch-thumb-background`, each on the token of the same name.

Midnight's variables map one-to-one: each one lands on the kyoko token of its own name (`--bg-4` on `bg_4`, `--accent-new` on `accent_new`, `--red-3` on `red_3`, `--gap` on `gap`, `--chatbar-height` on `chatbar_height`), and no Midnight variable feeds two tokens. That covers `--text-0`..`--text-5`, `--bg-1`..`--bg-4`, `--hover`, `--active`, `--active-2`, `--message-hover`, `--accent-1`..`--accent-5`, `--accent-new`, `--online`, `--idle`, `--dnd`, `--streaming`, `--offline`, `--border-light`, `--border`, `--border-hover`, `--button-border`, the `--red-n`, `--green-n`, `--blue-n`, `--yellow-n` and `--purple-n` scales, and every size of the mockups (`--gap`, `--border-thickness`, `--radius-*`, `--space-*`, `--guildbar-avatar-size`, ...). Seven sizes whose Midnight name oxidcord already used have their own token (`docs/ui.md`, "Midnight tokens"). Everything else follows through the derivations: oxidcord's tokens from Midnight's (`background_primary` from `bg_4`, `brand` from `accent_3`), the washes from their colour (`mention` is `accent_2` at 10%), the accents from the purple scale and the status colours from the base scales, the derived sizes from their inputs. So a Midnight-style file that only sets `--purple-2` and `--bg-4` restyles everything built on them, and importing Midnight's own variable block on any base gives exactly the built-in Midnight theme (`the_mockups_variable_block_restyles_kyoko_exactly_as_midnight`). A value the file sets itself always wins over a derivation.

`color-mix(in <space>, <colour> <n>%, transparent)` and mixes of two colours are read, so Midnight's washes import as well; gradients (`--mention`, `--reply`) are not, and take their colour from the derivation. Sizes are read when they are plain pixels (`12px`) or numbers; `calc()` is skipped and the size derived instead.

The mapping is `CSS_MAPPING` (colours: kyoko token, Discord names, Midnight names) and `CSS_SIZES` in `crates/bridge/src/themes/css.rs`; the derivations are `DERIVED_COLORS` and `DERIVED_SIZES` in `tokens.rs`. The first variable found wins: on a Midnight token its own variable, then Discord's names in order; on oxidcord's tokens Discord's names in order. A CSS file with none of these shows "No colours to import." on its card.

## The theme.json format

```json
{
	"name": "Violet Night",
	"author": "Example",
	"version": "1.0.0",
	"description": "Black with a violet accent.",
	"base": "amoled",
	"background_primary": "#000000",
	"brand": "#8b5cf6",
	"mention_background": "rgba(139, 92, 246, 0.28)",
	"radius_md": 12,
	"ui_fonts": ["Inter", "Segoe UI", "sans-serif"]
}
```

- `name`, `author`, `version`, `description`, `source` and `website` are optional details for the card. A theme folder's `theme.toml` takes their place when it has a `name`.
- `base` only matters for the first party themes in `themes/`, which are whole base themes: it picks the theme to start from, `dark` (default), `midnight`, `light` or `amoled`. A local or online theme goes on top of whichever base you chose, so its `base` is ignored.
- Colors use the CSS colour syntax above.
- Sizes are plain numbers in pixels at 100% interface scale. The interface zoom setting scales them.
- `ui_fonts` and `code_fonts` are lists of font family names, tried in order.
- Unknown or malformed tokens are ignored and logged; they never stop the theme from loading.

## Color tokens

Midnight's colour tokens (`bg_1`..`bg_4`, `text_0`..`text_5`, `accent_1`..`accent_5`, `accent_new`, `hover`, `active`, `active_2`, `message_hover`, the borders, status colours, base scales and washes) are listed in `docs/ui.md`, "Midnight tokens". oxidcord's, which the screens not yet restyled use:

| Group | Tokens |
| --- | --- |
| Surfaces | `background_primary` (main area), `background_secondary` (sidebars and panels), `background_tertiary` (deepest layer), `background_floating` (menus and popovers), `user_panel_background` |
| Interaction | `modifier_hover`, `modifier_selected`, `modifier_active`, `interactive_normal`, `interactive_hover`, `interactive_active`, `interactive_muted` |
| Text | `text_normal`, `text_muted`, `header_primary`, `header_secondary`, `text_on_brand` |
| Accent | `brand`, `brand_hover`, `link`, `mention_background`, `mention_text`, `unread_pill`, `mention_badge`, `speaking_ring`, `live_badge`, `brand_selected` (the active picker tab) |
| Status | `status_online`, `status_idle`, `status_dnd`, `status_offline`, `status_streaming`, `danger`, `danger_hover`, `success`, `warning` |
| Controls | `input_background`, `button_secondary`, `button_secondary_hover`, `divider`, `overlay`, `gif_shade` (the wash over GIF category tiles), `banner_button` (the connection banner's Retry) |
| Code and spoilers | `spoiler`, `code_background`, `code_border`, `scrollbar_thumb`, `scrollbar_track` |

## Size tokens

The ones worth changing:

| Tokens | What they do |
| --- | --- |
| `radius_xs`, `radius_sm`, `radius_md`, `radius_lg`, `radius_xl` | Corner radius from small (rows, chips) to large (panels, modals). Defaults 4, 8, 12, 16, 24 (Discord's scale) |
| `space_2` to `space_32` | The spacing scale used for gaps and padding |
| `font_tiny` to `font_xl` | Text sizes |
| `field_height`, `control_height`, `chip_height` | Height of inputs, buttons and chips |
| `channel_row_height`, `member_row_height`, `compact_row` | Density of the sidebars and compact messages |
| `message_avatar`, `message_gutter` | Message avatar size and the space around the message column |
| `server_rail_width`, `channel_sidebar_width`, `member_list_width` | Column widths of the oxidcord layout |
| `gap`, `border_thickness`, `guildbar_avatar_size`, `sidebar_width`, `member_list_panel_width`, `chatbar_height`, `channel_item_height` | Midnight's panels: the space between them, their borders, the server icon, column widths and row heights (`docs/ui.md`, "Midnight tokens") |

The complete list, with the defaults, is `COLORS` and `SIZES` in `crates/bridge/src/themes/tokens.rs`. Every key there is a valid token. `docs/ui.md` describes how the interface reads them.

## Share a theme

A theme is a file, so share the file or a direct link to it. There is no catalog server and no signing key: themes carry no code, so HTTPS and the checks above are all a link needs.

First party themes live in the top level `themes/` folder, one folder per theme, and are listed under Built In. They are not plugins: there is no manifest, no permissions and nothing runs.

```
themes/sunset/theme.json
themes/sunset/theme.toml
themes/sunset/README.md
```

`theme.toml` describes it:

```toml
id = "sunset"
name = "Sunset"
version = "0.1.0"
author = "Example"
description = "Warm dark theme."
```

The `id` matches the folder name and `[a-z0-9-]{1,64}`. Bump `version` whenever `theme.json` changes. `cargo xtask theme check` validates every folder. To add a theme, open a pull request that adds the folder; `themes/deep-blue` and `themes/amoled-violet` are complete examples.

## How it is built

- `crates/session/src/theme_links.rs` downloads the links (`Command::RefreshThemeLinks { urls }`), keeps the cache and publishes `Session::theme_links()` with `Changes::theme_links`. `session::cached_copies` reads the cache without the network.
- `crates/bridge/src/themes/` has the token tables, `Palette` and `Layer`, the CSS colour parser, the CSS importer (`import_css`, `parse_meta`), the link lines (`link_lines`, `theme_link`) and `Themes`, which loads the bases and the themes folder, takes the link copies and gives the page's entries and the layered palettes (`Themes::palettes`).
- `apps/desktop/src/ui/themes.rs` publishes the page, saves `settings.themes` (`enabled`, `links`), watches the folder and hands the layered palettes to `theme::Catalog::replace`, which re-registers the changed zgx themes so the window restyles at once.

## Tips

- Start from `amoled` for a black theme and from `dark` otherwise. Overriding five or six tokens is usually enough.
- Keep `text_normal` against `background_primary` above a 7:1 contrast ratio, and `text_muted` above 4.5:1.
- Raise the `radius_*` tokens together. A mix of tight and round corners looks accidental.
- Tint your greys toward the accent (a violet theme uses `#07050c`, not `#080808`), so the surfaces feel related to the brand color.
