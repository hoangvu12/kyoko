# Interface

Kyoko's interface is a 1:1 port of oxidcord 0.2.0-beta.1, written in Luau on zgx. The Rust host in `apps/desktop/src/` owns the window, the theme and everything that touches the session. The Luau scripts in `apps/desktop/ui/` lay out and draw. This page is the pattern every UI ticket follows. `docs/parity.md` says how a screen is compared with oxidcord.

## Where things live

```
apps/desktop/src/
  ui.rs            the window: Link (session to Luau), host actions, shared values, the Rust root layout
  theme.rs         oxidcord's theme tokens, theme files, zgx theme data
  fonts.rs         installed font families, picking the first of a font list
  frame.rs         the native window: maximized state, the exit waker
  timeline/        the message timeline, a Rust view in the timeline slot (below)
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
| Server rail | `server_rail.luau` | `server_rail_width` 72 | 22 (done) |
| Channel or DM sidebar, with the guild header | `channel_sidebar.luau` | `channel_sidebar_width` 240, top-left corner `radius_md`; hidden below 900 wide | 23, 24 |
| User panel at the bottom of the sidebar | `user_panel.luau` | `user_panel_height` 52 | 30 |
| Top bar: channel icon, name, topic, threads, notifications, pins, members, search, inbox, help | `top_bar.luau` | `top_bar_height` 48 with a 1-point bottom border | 20 (done); 31 search, 33 threads and pins, 35 notifications |
| Timeline | `timeline.luau` (an empty box) and `src/timeline/` | grows; reports its box as the timeline slot | 25 (done); 26 rich content |
| Composer | `composer.luau` | 44-point field, 16 sides, 24 below | 27 |
| Member list | `member_list.luau` | `member_list_width` 240, `background_secondary`; shown at 1200 wide and up, toggled by the members button | 29 |

The login screen (`login.luau`, ticket 21) stands beside the shell: `frame.luau` shows it instead of the shell while `login.screen` is not `main`, with the same title bar. `storage_notice.luau` is a warning bar under the title bar, shown only when `session.storage_warning` is set. See `docs/sign-in.md`.

Overlays (menus, popovers, dialogs, toasts, the lightbox, settings) are added beside the shell's root by the tickets that build them (28, 30–38).

### The timeline slot

zgx cannot place a Rust view inside a Luau tree, so the Rust root layout in `ui.rs` is an overlay: the Luau view fills the window, and a Rust view is absolutely positioned above it at the box the Luau timeline reports. `shell/timeline.luau` is an empty box that calls `actions.report_slot("timeline", element)` from `on_resize`. The host function `frame.slot` turns that into the `Slot` signal `timeline_slot` follows, and tells the timeline its size. Because the Rust view sits above the whole Luau tree, a Luau overlay (menu, popover, dialog, lightbox) that covers the timeline's box is drawn under it: a ticket adding one either draws it in Rust above the slot or hides the slot while it is open.

## The timeline

`src/timeline/` is the message list. `bridge::Timeline` projects the open channel's `State::channel_rows` into keyed rows (`RowKey`: `Loading`, `Start`, `Day(date)`, `Unread`, `Message(id)`, `Pending(nonce)`) and publishes what changed since the last publish as a `bridge::Delta`: `reset` (a channel switch or a wholesale reload), `prepended` and `appended` counts, and `inserted`, `updated` and `removed` keys. `Link` hands the rows and the delta to `TimelineView::apply`; nothing goes through a shared value.

- Rows: zgx's `measured_virtual_list` over the keys. Heights are kept by key across inserts and removals, rows not yet mounted take an estimate from their text (`rows::estimate`), and mounted rows are measured. Each key has a signal holding its `Row`, so an edit restyles one row. `rows::render` has one renderer per row kind (skeleton, welcome, day separator, unread marker, first message of a group, follow-up, system message); ticket 26 extends the message body there.
- Follow: `VariableHeights::anchor_end` keeps a viewport resting at the end there when rows are added or measured. A channel switch scrolls to the end.
- Anchoring: when the rows change and the reader is not at the bottom, the view remembers the first visible rows by key and their distance from the viewport top, and after the change sets the offset so the first of them that survived sits exactly where it was. zgx re-anchors only point height changes, so this covers prepends, inserts and removals above (`scroll::Anchor`).
- Paging: within 14 rows of the top (or with fewer than 14 rows), while the channel has older history, the view sends `LoadOlder`; while the window is detached and the reader is at the bottom it sends `LoadNewer`. Each is sent once per edge message, again after 3 seconds (`scroll::Paging`); the session also drops a duplicate in-flight page.
- Viewport: the first and last visible message ids and whether the reader is at the bottom (within 2 points) go to the session as `SetViewport`, at most once per 250 ms and only when they change (`scroll::Reporter`). The session decides acks from it; the timeline never acks.
- Images: `timeline::Pictures` is every image the rows show, through the gallery: avatars (`Circle`, 40 points; 16 in reply previews), custom emoji (22, 48 when jumbo, 16 in reactions), attachments fitted to 550 by 350, embed images (400 by 300, thumbnails 80, author icons 24), stickers (160). `Pictures::names_in` lists an entry's images the same way the rows request them, so the images of the visible rows (and two rows either side) are merged with the user panel's avatar, the rail icons and the Luau `media.visible` set into one `Gallery::set_visible` call: animated emoji, GIFs and animated WebP play only on screen, as the gallery's playback and reduced-motion settings allow. In mock runs every URL that is not the mock server's goes to its `/media/<hash of the path>.png`, as oxidcord's mock resolver does, and the mock fetcher refuses remote hosts. Attachments and embeds show their thumbhash placeholder until the image arrives.
- Text in the list is selectable and copies with Ctrl+C (`selectable`); one selection runs across a message's paragraphs and copies plain text, mentions as their names. Timestamps are local time, formatted as oxidcord does (`bridge::message_time`, `clock_text`, `day_label`, `timestamp_text` for `<t:..>`).

### Message content

`bridge::Timeline` lowers each message once into its `MessageEntry`: `body` (the markdown of the content, embed descriptions and field values, parsed once per message revision and cached), `names` (every user, role and channel mentioned, resolved through state; an unresolved user reads `@unknown-user` until the session fetches it), `reply` (`ReplyPreview::{Message, Deleted, Unavailable, Loading}` from `State::reply_target`; `Loading` without Read Message History reads as unavailable) and `system` (the line and icon of a system message). A resolved mention or reply changes the entry, so only that row redraws.

The row renderers are in `src/timeline/`: `rows.rs` (the frame of each row kind, reply previews, system rows with oxidcord's icons, deleted styling: muted text with a red `(deleted)`), `body.rs` (markdown: one zgx `rich_text` paragraph per run of inline content, blocks for headings, subtext, quotes, lists and code), `inline.rs` (custom emoji: a padded word-joiner placeholder in the paragraph, with the image laid over its shaped rectangle after layout, so wrapping and selection stay one paragraph), `attachments.rs` (images, file and video cards, embeds, stickers; Lottie stickers draw nothing, as in oxidcord), `extras.rs` (reactions, polls, thread chips) and `links.rs`. Code blocks are highlighted by the `markdown` crate's own highlighter. Unicode emoji are text in the system emoji font, as oxidcord draws them, so no emoji image set is bundled.

Interactions owned here: spoilers reveal on click (per message, kept while the window lives), poll answers vote (`VotePoll`; a multi-select poll collects picks and sends them with Vote; the author can End Poll), embed titles, author links and video embeds open their links, file cards open the file. A link opens at once for oxidcord's trusted hosts and the user's `trusted_link_hosts`; any other host first shows the "Leave Kyoko?" dialog (a zgx modal from the timeline view, so it sits above the slot), whose "Always trust" saves the host to settings. Opening goes through zgx's `open_url`. Reacting, the lightbox, profiles and jump to message belong to tickets 28, 32, 30 and 28.

Labels oxidcord leaves at gpui's default line height are given 1.618 times their size (`TypeSize::type_size`), so cards and embeds stack to oxidcord's heights.

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

## Server rail

`shell/server_rail.luau` is oxidcord's rail: home, a divider, servers and folders in layout order, add a server and discover. Each square is `components/rail_tile.luau` (oxidcord's tile: the unread pill from `components/unread_pill.luau` at 8, 20 or 40 points for unread, hovered and selected, the 140 ms morph between a 24-point circle and a 16-point rounded square, the mention badge from `components/mention_badge.luau`, a tooltip). `components/rail_gap.luau` is the drop strip in the 8 points above a square.

Rust publishes `rail.items` from `bridge::Rail` (`apps/desktop/src/rail.rs`): a list of `{kind = "guild", key, id, name, acronym, letter, icon, unread, mentions}` and `{kind = "folder", key, id, name, color, tint, unread, mentions, guilds}`, recomputed when the guild list, read state, settings or DMs change and published only when it differs. Unread leaves out muted servers. Folder names default to the first three server names, colours to `#5865f2`, and `tint` is the colour at 40%. `rail.home_mentions` sums the DM mentions. `rail.open_folders` lists the open folder ids; the host keeps them in the store (`rail.open_folders`), and selecting a server opens its folder, as oxidcord does.

Server icons are `bridge::guild_icon_request` at 48 points, `Square`, published as `host:` names and kept visible. The tile clips them: a node with `rounded` and `overflow = "hidden"` cuts every descendant, images included, to its rounded corners, so one square image follows the morph and the folder grid's 16-point circles without masked variants.

Dragging a server or a folder calls `rail.drop` with `{ guild | folder, drop, target }`, where `drop` is `before_guild`, `onto_guild`, `before_folder`, `onto_folder` or `end`. `bridge::rearranged` and `bridge::rearranged_folder` work out the new layout (a server dropped on another makes a folder; the last server leaving a folder removes it; folders never nest) and the host sends `Command::SetGuildLayout`. How it is saved: "Server order and folders" in `docs/rest.md`.

## Channel sidebar

`shell/channel_sidebar.luau` is oxidcord's channel sidebar. The guild header is a 48-point button with the server name and a chevron, centred, that opens the server menu. Categories are oxidcord's section header: a 12-point chevron (right when collapsed), the name in capitals in `header_secondary` (`interactive_hover` on hover), and a create-channel button shown on hover. Channel rows are 32 points high with an icon by kind (`hash`, `megaphone`, `volume`, `stage`, `forum`, `thread`), a lock badge on channels the user cannot view, the white unread bar at the left edge, the mention badge, `modifier_selected` when open and `modifier_hover` on hover. The name and icon are `interactive_active` when open, `header_primary` when unread, `interactive_muted` when muted or locked and `text_muted` otherwise; on hover only the icon brightens, as in oxidcord. Threads sit 40 points in under their channel with oxidcord's spine. Voice and stage channels list who is in them: a 24-point avatar, the name, and the camera, screen, mute or deafen icon.

Rust publishes the selected guild's tree from `bridge::Sidebar` (`apps/desktop/src/sidebar.rs`). `sidebar.rows` is a flat list keyed by `key`: `{kind = "category", key, id, name, collapsed, muted}` and `{kind = "channel", key, id, type, icon, name, unread, mentions, muted, locked, joined, selectable, thread, last_thread, voice}`, with `voice` a list of `{key, id, name, avatar, muted, deafened, video, live}`. A collapsed category keeps the open channel, unread channels and voice channels with people in them, as oxidcord does. The projection is recomputed when the guild's channels, read state, guild settings or voice states change, and published only when it differs, so zgx rebuilds only the rows that changed. `sidebar.header` holds the server menu's flags (`unread`, `muted`, `owner`, `can_invite`, `can_manage_channels`, `can_open_settings`). Voice avatars are `Circle` gallery images at 24 points, kept visible.

Clicking a text, announcement, voice or stage channel or a thread opens it in the timeline (`sidebar.select_channel`, which sends `Command::SelectChannel`; the session subscribes and loads it). A voice channel opens its text chat without joining voice. Forum channels are listed but do not open yet (33), and locked channels do nothing. Clicking a category sends `Command::ToggleCategory`; how it is saved: "Collapsed categories" in `docs/rest.md`.

The server menu opens under the header and the channel menu opens under the row it belongs to (`context_menu_below`), both 224 points wide, so neither reaches the message area and the overlay rule does not apply. "Mark As Read" and "Copy Link" work; the other items call actions that log until 35 and 36 fill them in.

## Shared values and host actions

Names are `<area>.<name>` in snake case, with the area the region or service that owns them. Plugins (45) will get `plugin.<id>.<name>`.

| Kind | Names |
|---|---|
| Shared values (Rust publishes, Luau reads) | `login.screen`, `login.acknowledged`, `login.checking`, `login.message`, `login.error`, `login.unsaved`, `session.status`, `session.storage_warning`, `rail.items`, `rail.home_mentions`, `rail.open_folders`, `sidebar.rows`, `sidebar.header`, `sidebar.guild_id`, `sidebar.guild_name`, `timeline.channel_id`, `timeline.channel_name`, `timeline.channel_topic`, `user.avatar`, `user.avatar_ready`, `window.maximized` |
| Actions (`ui.action`, fire and forget) | `login.acknowledge`, `login.submit`, `login.continue`, `login.sign_out`, `rail.select_guild`, `rail.select_home`, `rail.toggle_folder`, `rail.drop`, `rail.add_server`, `rail.discover`, `sidebar.select_channel`, `sidebar.toggle_category`, `sidebar.mark_channel_read`, `sidebar.mark_guild_read`, `sidebar.copy_channel_link`, `sidebar.invite`, `sidebar.server_settings`, `sidebar.create_channel`, `sidebar.notification_settings`, `sidebar.mute_guild`, `sidebar.mute_channel`, `sidebar.edit_channel`, `sidebar.delete_channel`, `sidebar.leave_guild`, `window.minimize`, `window.maximize`, `window.close`, `media.copy_image` |
| Functions (`ui.call_sync`) | `media.visible`, `media.hover`, `media.reduced_motion`, `frame.slot` |

Rust registers them in `Link::install` (and `Integration::install` for desktop integration). Luau declares each shared value once in `state.luau` and wraps each action in `actions.luau`, so a rename touches one Luau file. State only the interface cares about stays in Luau: `ui.state` in the module that owns it, or in `state.luau` (`state.shell.members_visible`) when several modules share it. Never publish the whole store. Publish small projections from `bridge` and let zgx reconcile them.

## Window frame

The window is frameless (`Titlebar::Hidden`): Windows keeps its shadow, rounded corners and resize edges, and the title bar is kyoko's own, as oxidcord's is. The drag region is `window_drag = true`: a double click maximizes and a right click opens the system menu. The buttons call `window.minimize`, `window.maximize` and `window.close`. `frame.rs` finds the native window to read whether it is maximized (`window.maximized` swaps the maximize and restore icons). Windows 11 snap layouts on the maximize button need zgx's caption hit testing, which only `Titlebar::Inset` has, so they are missing for now.

## Adding a screen

1. Add the module under `shell/` (or `components/` if it is reusable), styled only from `theme.luau`, with icons by name.
2. Publish what it needs from Rust as `<area>.<name>` shared values, declare them in `state.luau`, and add its actions to `actions.luau`.
3. Add its capture steps (`docs/parity.md`) and compare `pair.png` with oxidcord.
