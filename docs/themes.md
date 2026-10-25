# Themes

A theme changes colours, and a kyoko `theme.json` can change sizes and fonts too. There is no code and nothing runs, so a theme cannot do anything except change how Kyoko looks.

Settings, Themes (in the Kyoko section, beside Plugins) lists three kinds:

- **Built In**: Dark, Light, AMOLED and the first party themes in the repository's `themes/` folder. Apply makes one the base theme. Appearance's Dark, Light, AMOLED and System choices write the same setting (`theme`).
- **Local Themes**: the files in your themes folder, each with a switch.
- **Online Themes**: links you add, one per line.

Enabled local themes and every online theme apply on top of the base theme, live: local themes first, in the order you turned them on, then online themes in the order of their lines. A later theme wins where two set the same colour. Turning a theme off (or deleting its file, or removing its line) brings back the colours underneath at once.

Each card shows the theme's name, version, author and description, a strip of six of its colours as they look on your base theme, and any error.

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

Vencord and BetterDiscord themes are CSS written for Discord's web page, so most of a theme (layouts, fonts, images, animations, anything aimed at Discord's class names) cannot apply to Kyoko. Kyoko keeps only the colours:

- It reads the custom properties (`--name: value`, Discord's variable names and Midnight's) declared on `:root`, `html`, `body`, `#app-mount`, `.theme-dark`, `.theme-light`, `.visual-refresh` and similar whole-app selectors, including inside `:is()`, `:where()` and `@media` blocks. Properties scoped to a part of the page (`[class*="sidebar"] { ... }`) and every ordinary declaration are ignored.
- Properties declared under `.theme-light` (or `prefers-color-scheme: light`) apply only on a light base, those under `.theme-dark` only on a dark one.
- `var(--x)` and `var(--x, fallback)` are resolved inside the file, so a theme built on its own variables (`--accent: hsl(var(--hue) 60% 40%)`) imports correctly.
- Colours may be hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`), `rgb()`/`rgba()`, `hsl()`/`hsla()` (comma or space syntax, `/ alpha`, angle units) and common colour names. `!important` is dropped. Anything else (`calc()`, `color-mix()`, gradients, images, an unresolved variable, a cycle) is skipped, never fatal.
- The details come from the first `/** ... */` comment, one `@tag value` per line, the BetterDiscord format: `@name`, `@author`, `@description`, `@version`, `@source`, `@website`. The name defaults to the file name.

| Kyoko token | Discord variables (Vencord, BetterDiscord) | Midnight variables |
| --- | --- | --- |
| `background_primary` | `--background-primary`, `--background-base-low` | `--bg-4` |
| `background_secondary` | `--background-secondary`, `--background-base-lower` | `--bg-4` |
| `background_tertiary` | `--background-tertiary`, `--background-base-lowest` | `--bg-3` |
| `background_floating` | `--background-floating`, `--background-surface-highest` | `--bg-3` |
| `user_panel_background` | `--background-secondary-alt` | `--bg-4` |
| `modifier_hover` | `--background-modifier-hover` | `--hover` |
| `modifier_selected` | `--background-modifier-selected` | `--active` |
| `modifier_active` | `--background-modifier-active` | `--active-2` |
| `message_hover` | `--background-message-hover` | `--message-hover` |
| `text_normal` | `--text-normal`, `--text-default` | `--text-3` |
| `text_muted` | `--text-muted` | `--text-4` |
| `header_primary` | `--header-primary`, `--text-strong` | `--text-2` |
| `header_secondary` | `--header-secondary`, `--text-subtle` | `--text-4` |
| `interactive_normal` | `--interactive-normal` | `--text-4` |
| `interactive_hover` | `--interactive-hover` | `--text-3` |
| `interactive_active` | `--interactive-active` | `--text-1` |
| `interactive_muted` | `--interactive-muted` | `--text-5` |
| `text_on_brand` | `--white-500` | `--text-0` |
| `brand` | `--brand-experiment`, `--brand-500`, `--brand-experiment-500` | `--accent-3` |
| `brand_hover` | `--brand-experiment-560`, `--brand-560` | `--accent-4` |
| `link` | `--text-link` | `--accent-1` |
| `mention_background` | `--mention-background` | — |
| `mention_text` | `--mention-foreground` | `--accent-1` |
| `unread_pill` | — | `--accent-2` |
| `mention_badge` | — | `--accent-new` |
| `status_online` | `--status-online`, `--status-positive` | `--online` |
| `status_idle` | `--status-idle`, `--status-warning` | `--idle` |
| `status_dnd` | `--status-dnd`, `--status-danger` | `--dnd` |
| `status_offline` | `--status-offline` | `--offline` |
| `status_streaming` | — | `--streaming` |
| `danger` | `--status-danger`, `--text-danger` | `--red-3` |
| `danger_hover` | `--button-danger-background-hover` | `--red-4` |
| `success` | `--status-positive`, `--text-positive` | `--green-2` |
| `warning` | `--status-warning`, `--text-warning` | `--yellow-2` |
| `scrollbar_thumb` | `--scrollbar-auto-thumb`, `--scrollbar-thin-thumb` | `--bg-1` |
| `scrollbar_track` | `--scrollbar-auto-track` | — |
| `divider` | `--background-modifier-accent` | `--active` |
| `input_background` | `--input-background`, `--channeltextarea-background` | `--bg-3` |
| `code_background` | — | `--bg-3` |
| `button_secondary` | `--button-secondary-background` | `--bg-2` |
| `button_secondary_hover` | `--button-secondary-background-hover` | `--bg-1` |
| `spoiler` | `--spoiler-hidden-background` | — |
| `focus_ring` | `--focus-primary` | — |

The first variable found wins, Discord's names before Midnight's. Midnight (refact0r's theme, `docs/design/midnight.md`) names colours by role (`--bg-1`..`--bg-4`, `--text-0`..`--text-5`, `--accent-1`..`--accent-5`, `--hover`, `--active`, status colours and the `--red-n`, `--green-n`, `--yellow-n` scales); each maps onto today's nearest token, so several Midnight variables can share one (`--bg-4` is every panel). The mapping is the one table `CSS_MAPPING` in `crates/bridge/src/themes/css.rs`; Midnight ticket M01 makes Midnight's variables kyoko's own tokens and can turn its Midnight column one-to-one there. A CSS file with none of these shows "No colours to import." on its card.

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
- `base` only matters for the first party themes in `themes/`, which are whole base themes: it picks the theme to start from, `dark` (default), `light` or `amoled`. A local or online theme goes on top of whichever base you chose, so its `base` is ignored.
- Colors use the CSS colour syntax above.
- Sizes are plain numbers in pixels at 100% interface scale. The interface zoom setting scales them.
- `ui_fonts` and `code_fonts` are lists of font family names, tried in order.
- Unknown or malformed tokens are ignored and logged; they never stop the theme from loading.

## Color tokens

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
| `radius_sm`, `radius_md`, `radius_lg`, `radius_xl` | Corner radius from small (rows, chips) to large (modals, cards). Defaults 4, 8, 16, 24 |
| `space_2` to `space_32` | The spacing scale used for gaps and padding |
| `font_tiny` to `font_xl` | Text sizes |
| `field_height`, `control_height`, `chip_height` | Height of inputs, buttons and chips |
| `channel_row_height`, `member_row_height`, `compact_row` | Density of the sidebars and compact messages |
| `message_avatar`, `message_gutter` | Message avatar size and the space around the message column |
| `server_rail_width`, `channel_sidebar_width`, `member_list_width` | Column widths |

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

The `id` matches the folder name and `[a-z0-9-]{1,64}`. Bump `version` whenever `theme.json` changes. `cargo xtask theme check` validates every folder. To add a theme, open a pull request that adds the folder; `themes/midnight` and `themes/amoled-violet` are complete examples.

## How it is built

- `crates/session/src/theme_links.rs` downloads the links (`Command::RefreshThemeLinks { urls }`), keeps the cache and publishes `Session::theme_links()` with `Changes::theme_links`. `session::cached_copies` reads the cache without the network.
- `crates/bridge/src/themes/` has the token tables, `Palette` and `Layer`, the CSS colour parser, the CSS importer (`import_css`, `parse_meta`), the link lines (`link_lines`, `theme_link`) and `Themes`, which loads the bases and the themes folder, takes the link copies and gives the page's entries and the layered palettes (`Themes::palettes`).
- `apps/desktop/src/ui/themes.rs` publishes the page, saves `settings.themes` (`enabled`, `links`), watches the folder and hands the layered palettes to `theme::Catalog::replace`, which re-registers the changed zgx themes so the window restyles at once.

## Tips

- Start from `amoled` for a black theme and from `dark` otherwise. Overriding five or six tokens is usually enough.
- Keep `text_normal` against `background_primary` above a 7:1 contrast ratio, and `text_muted` above 4.5:1.
- Raise the `radius_*` tokens together. A mix of tight and round corners looks accidental.
- Tint your greys toward the accent (a violet theme uses `#07050c`, not `#080808`), so the surfaces feel related to the brand color.
