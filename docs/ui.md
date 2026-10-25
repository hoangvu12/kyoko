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

### The load check

`cargo xtask test` and `cargo test --workspace` check the scripts headless, with no window and no network, in under a second once built (`src/ui/scripts.rs`). Run it alone with `cargo test -p kyoko --bin kyoko ui::scripts`.

- `every_script_compiles`: every `.luau` under `ui/` must parse. A syntax error fails with the file and line (`actions.luau:6: Expected 'end' ...`).
- `the_interface_loads_and_builds_every_screen`: `app.luau` loads in zgx's `testing::TestApp` with the real theme installed and a stub action and function for every `"area.name"` string literal in `src/`, then builds the terms notice, the token form, the DMs, a server with its channels and members, every settings page, and a home page from sample shared values. Each screen must show its marker text. A missing `require`, an unregistered host name or a runtime error fails it with the script's traceback and the screen it was building.
- `the_typing_dots_animate_only_while_the_setting_allows`: reads the opacity of the composer's `typing_dot_*` nodes over real time and checks they cycle with `chat.typing_animation` on and stay lit with it off.

To stay loadable, a new module must:

1. Parse, and require only modules that exist (`./`, `../`, `@components/`, `@shell/`).
2. Call the host only through names Rust registers with a string literal (`host.action("area.name", ...)`, or a literal list it loops over); a name built at runtime is not stubbed.
3. Build without error when every shared value is at its `state.luau` default, since the check starts from those.
4. If it is a new screen or region, get sample shared values and a marker text in `scripts.rs`, so the check builds it too.

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
| Composer | `composer.luau` | 44-point field, 16 sides, the 24-point typing line below | 27 (done) |
| Member list | `member_list.luau` | `member_list_width` 240, `background_secondary`; shown at 1200 wide and up, toggled by the members button | 29 (done) |

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

Interactions owned here: spoilers reveal on click (per message, kept while the window lives), poll answers vote (`VotePoll`; a multi-select poll collects picks and sends them with Vote; the author can End Poll), embed titles, author links and video embeds open their links, file cards open the file. A link opens at once for oxidcord's trusted hosts and the user's `trusted_link_hosts`; any other host first shows the "Leave Kyoko?" dialog (a zgx modal from the timeline view, so it sits above the slot), whose "Always trust" saves the host to settings. Opening goes through zgx's `open_url`. The lightbox belongs to ticket 32; profiles are below.

### Message actions

`bridge::message_menu` decides which actions a message offers, in oxidcord's order: Reply, Edit Message (own), Copy Text, Copy Link, Forward, Copy ID (developer mode), Pin or Unpin Message (Manage Messages, or any DM), Create Thread, Mark Unread, Select or Deselect Message (own messages, or Manage Messages), Delete N Selected, Delete Message (own, or Manage Messages) and Report Message (others'), with oxidcord's two separators. It reads a `bridge::Access` (the open channel's permissions and safety lock), which `Link` hands the timeline on every publish. `bridge::toolbar_actions` gives the hover bar (Add Reaction, Reply, Edit). System, pending and deleted messages get no actions; a kept deleted message only copies. Links are `https://discord.com/channels/<guild or @me>/<channel>/<message>`.

`src/timeline/actions.rs` draws them, all from Rust in the timeline view, so they sit above the slot (the overlay rule):

- The context menu opens on a right click as a zgx `menu` anchored at the pointer, 188 points wide with 28-point items, a 12-point icon and danger items in red. zgx keeps popups inside the screen, so the anchor is moved up and left first to keep the menu inside the window, as oxidcord's does. zgx draws menus, popovers and tooltips in separate popup windows on Windows; the parity capture composites them (`docs/parity.md`).
- The hover bar shows on the row under the pointer (and while its More menu is open, hidden while its context menu is open): Add Reaction, Reply, Edit and More, which opens the same menu.
- Delete, Delete N Selected, Pin and Unpin confirm in a zgx modal with oxidcord's wording (`dialogs.rs`). Shift while choosing Delete Message skips the confirmation, as the web client does.
- Selecting: Shift-click a message, or click one while any is selected, or use Select Message. Selected rows take `message_selected`. A bar at the bottom shows the count with Cancel and Delete; Delete confirms and sends `DeleteMessages` (paced single deletes, never bulk delete), and the bar then follows the batch from `State::deletions()`: "Deleting 2 of 3…", then what was deleted, refused and why the rest stopped. Escape clears the selection.
- Alt-click a message marks it unread (`MarkUnread`).
- Reactions: a reaction chip toggles your reaction; Add Reaction opens the emoji picker (`picker.rs`, oxidcord's emoji tab: search, a rail of recent, server and Unicode sections, a preview footer and skin tones; Enter in the search picks the first result) as a popover beside the pointer. A new emoji needs Add Reactions, joining an existing one only Read Message History. When the channel has a safety lock the reaction is held and the bar shows the lock's explanation with "React anyway", which sends `ConfirmRisky` then the reaction; for a gate it only explains.
- Failures the session reports (`CommandRejected`, such as a refused pin or reaction) show in the same bar for a while after the user acted.
- Reply and Edit hand the message to the composer (`Compose::Reply`, `Compose::Edit`, through `Link::composer_reply` and `Link::composer_edit`). Clicking a reply preview jumps to the original: `TimelineView::jump_to` scrolls to it when it is loaded, otherwise sends `LoadAround` and scrolls once the reset arrives. Clicking an avatar, an author name or a user mention calls the profile hook (`TimelineView::on_profile`), which opens the profile popout at the pointer.
- Forward and Create Thread call stubs that log until tickets 35 and 33; Report Message opens Discord's report page, as oxidcord does.

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

## DM sidebar

Selecting Home (`rail.select_home`) clears `sidebar.guild_id`, and `channel_sidebar.luau` then shows `shell/dm_sidebar.luau` in place of the guild's channels: oxidcord's "Find or start a conversation" bar, the Friends, Nitro and Shop rows, a Message Requests row with its count when there are any, the "Direct Messages" header with its create button, and one 42-point row per conversation. A 1:1 row shows the recipient's avatar (`components/avatar.luau`, a 32-point `Circle` gallery image) with the status dot from `components/status_dot.luau`; a group row shows the brand circle with the group icon and "N Members" under the name. The name is semibold and `header_primary` when unread, `interactive_active` when open; the mention badge sits before the close button, which shows on hover.

Rust publishes `dms.rows` from `bridge::DmList` (`apps/desktop/src/dm_sidebar.rs`): `{key, id, name, avatar, status, group, members, unread, mentions}`, ordered by the last message id (the channel id for an empty conversation), so a new message or a conversation that arrives late moves to the top at once. Message requests and spam (`is_message_request`, `is_spam`) are left out of the list and counted in `dms.requests`. Statuses come from presence state; a recipient without a presence shows offline. A group without a name is named after its recipients.

Home opens the last conversation opened there, or the most recent one, in the timeline (`SelectChannel` with no guild); clicking a row does the same (`dms.select`). The top bar then shows `at_sign`, the conversation's name and status, and the call, video, pins, search, inbox and help buttons; the member list is hidden. Friends, Nitro, Shop and Message Requests set `home.page`, which covers the main column with a placeholder (ticket 34 builds the pages) and collapses the timeline slot until a conversation or a server is opened. The close button opens a confirmation under it, inside the sidebar, worded as oxidcord's ("Close Conversation", or "Leave Group" for a group); confirming sends `Command::CloseDm`, which is `DELETE /channels/{id}`. `dms.search` and `dms.create_group` log until 31 and a group-DM ticket fill them in.

## Composer

`shell/composer.luau` is oxidcord's composer: the attach button in a 56-point leading box, the text area, the GIF, sticker and emoji buttons, and the 24-point typing line ("X is typing...") below. It adds what the web client has and oxidcord lacks: the ping toggle on the reply bar (`@ ON`/`@ OFF`), an edit bar, a character count once fewer than 200 characters are left, and a notice bar for locks, refusals and failed sends.

- Text: a `ui.textarea` with `wrap` and `spellcheck` (Windows spell checking through zgx, with guesses in its context menu). IME composition is zgx's. The height follows a hidden copy of the text, from one 20-point line to ten. Enter sends, Shift+Enter breaks the line, Up in an empty composer edits your last message, Escape closes the suggestions, then the edit, then the reply. Ctrl+V attaches copied files or a copied image before falling back to text.
- Logic lives in `bridge::Composer` (no UI dependency): the reply and its ping, the edit, the attachments, autocomplete, slash parsing, the lock and the notice. `apps/desktop/src/ui/composer.rs` wires it to the window: it publishes `composer.*`, reads files off the UI thread, makes upload thumbnails and saves the draft 500 ms after the last keystroke.
- Autocomplete reads `State` only: `@` members (the member list, cached members, recent authors, DM recipients; two or more letters also ask the member scheduler with `SearchMembers`), mentionable roles, `@everyone` and `@here`; `#` channels; `:` server and Unicode emoji from two letters; `/` the application command index at the start of the message. Up and Down move, Tab or Enter take, a click takes.
- Files: the `+` button (`ui.open_files`), Ctrl+V, and drag-and-drop on the window (winit's OLE drop target, `View::on_files_drop` on the Rust root, the hover shown as a brand wash). Images get a 96-pixel thumbnail (`host:composer/<id>`) in their chip. The size and count limits are checked before a file joins the tray.
- Locks (`docs/locks.md`): while `State::channel_lock` holds, the notice shows `RiskyReason::explanation()` and "I understand" (`ConfirmRisky`); a server gate has no confirm button, its explanation sends the user to the official client; a held send is re-sent after the confirmation. The lock is re-read on every change that can lift it and at its `until`.
- Overlay rule: everything the composer adds above the field (suggestions, notice, reply and edit bars, the file tray) takes layout space, so the timeline slot shrinks and moves up rather than covering them. The timeline stays pinned to its end when the slot changes height while it was at the bottom.
- Entry points for message actions (28): `composer.start_reply(message_id)` and `composer.start_edit(message_id)`.

## Member list

`shell/member_list.luau` is oxidcord's member list: the "Search members" field at the top, then the list in a zgx virtual list (`ui.list` with per-row `heights`), so only the rows on screen are built. Group headers are oxidcord's ("ONLINE — 39", a role's name and count, "SEARCH RESULTS - N"): 12-point semibold capitals in `header_secondary`, 24 points above and 4 below. Member rows are 42 points high: the 32-point avatar (`components/avatar.luau`, a `Circle` gallery image) with its status dot, the name in the colour of the highest coloured role (`text_normal` otherwise, `text_muted` when offline), the BOT tag, the override badge and the activity line (Playing, Listening to, Watching, Streaming, Competing in, or the custom status), `modifier_hover` on hover; offline rows are at 50% opacity. Rows not loaded yet are placeholders (a circle and a bar) that fill in when their range arrives. The list is its own column, toggled by the top bar's members button, shown at 1200 wide and up, and hidden in DMs. Clicking a row calls `members.open_profile`, which goes to `Link::open_member_profile` in `src/ui/members.rs`.

Rust projects it in `bridge::MemberPanel` (`crates/bridge/src/member_list.rs`) and publishes it from `apps/desktop/src/member_list.rs`. The layout (a group header or a member for every row of `State::member_list`) comes from the list's groups, corrected by the slots that have loaded; `members.heights` holds each row's height and changes only when the layout does. `members.window` holds the rows on screen and 12 either side, `{ first, rows }`, each `{ kind = "group", key, title }`, `{ kind = "member", key, id, name, color, status, activity, bot, badge, avatar }` or `{ kind = "placeholder", key, index }`. Each row is a `ui.presence` over its record, so a row rebuilds only when its record changes. `members.jump`, `{ serial, offset }`, scrolls the list from Rust; capture mode uses it to scroll to the end.

Clicking a row calls `members.open_profile` with the pointer position, which opens the profile popout to the left of the pointer (below).

Member-list updates are coalesced apart from the timeline: a change to the guild's list, the guild list, or the presence of anyone in the window only marks the panel dirty, and a task publishes at most once every 150 ms (`MEMBER_PUBLISH_GAP`). The Luau list reports its scroll offset and height with `members.scroll`; Rust turns them into the visible rows, re-centres the window when the viewport nears its edge, and sends `Command::SetMemberViewport` when the visible rows change. The session's planner turns that into op 37 ranges; the interface never talks to the gateway. Opening a channel scrolls the list back to the top, as the session resets that guild's ranges. Typing in the search field filters the loaded members at once, as oxidcord does, and after 350 ms sends `Command::SearchMembers` when the guild has members not loaded yet. Member avatars of the window are part of `Gallery::set_visible`.

## Settings

`shell/settings.luau` is oxidcord's settings view: it covers everything under the title bar, with the page list on the left (218 points, right-aligned in a `background_secondary` panel), the page on the right (at most 740 points) and the round close button with "ESC" at the top right. Escape closes it too. It opens from the user panel's settings button through `settings.open` with a page key, and `settings.close` closes it.

- Page registry: `ui/settings/pages.luau` holds `sections`, each `{ key, title?, pages }`, and every page is `{ key, title, icon, build }`. `pages.add_page(section_key, page)` and `pages.add_section(section)` extend it: ticket 45 adds a "Plugins" section and plugin panes, 57 a Themes page, 39 translates the titles. The host accepts any page key made of lowercase letters, digits, `-`, `.` and `_`. The voice and video, updates and WASM plugin pages are left out.
- Pages: My Account (`settings.account`: name, username, email and phone masked as the web client does, banner colour, avatar and status; Password and Two-Factor open Discord's web app; Log Out is `login.sign_out`), Profiles, Devices, Connections, Clips and Friend Requests (notes, since kyoko writes none of these), Privacy & Safety (the safety locks, the link warning and the trusted sites), Appearance, Accessibility, Chat, Notifications, Keybinds, Language, Streamer Mode and Advanced. `ui/settings/widgets.luau` has oxidcord's heading, row, switch, radio group, slider, buttons and note card.
- Values: `bridge::SettingKey` names every setting the pages change (`chat.inline_media`, `media.animation`, `keybinds.toggle_window` and so on). `settings.values` publishes them all as one map; `settings.set { key, value }` checks the value (`bridge::change_setting`: unknown keys and bad values change nothing, numbers are clamped, hosts are trimmed and lowercased), saves `settings.json` through `Store::update_settings` and applies it. `settings.notes` carries messages by key (a keybind that could not be registered, the test notification).
- Applied at once: theme (`Catalog::apply`), reduced motion (with the system's), animation playback, image quality, inline media, link previews, reactions, the link warning and trusted sites (`TimelineView::set_prefs` re-renders the rows), the typing dots, developer mode (Copy ID in 28's menus), streamer mode on the account card, notifications and the tray (read when used), the show/hide keybind (re-registered by `Integration`), start at login and the discord:// handler (`apply_startup_settings`). After a restart: safety locks and language. Saved for later versions: compact messages, DPI and font scaling, saturation, 24-hour time, emoji autocomplete, hiding invites and GIF usage.
- Overlay rule: while a page is open the host collapses the timeline slot, as the home pages do, and gives it back on close unless a home page still covers it.

Registering a page: whoever owns the page adds it to the registry before `shell/frame.luau` builds the shell, because `shell/settings.luau` builds the page list once, then. The `@settings` alias reaches `ui/settings/`. A page shows its `title` above what `build` returns, `build` runs each time the page opens, and the page opens with `actions.open_settings(key)`; capture steps reach it by mapping a screen name to the key in `settings_page` in `src/capture.rs`. Add the key and a text only that page shows to `SETTINGS_PAGES` in `src/ui/scripts.rs`, so the load check builds it.

```lua
local pages = require("@settings/pages")
local widgets = require("@settings/widgets")

pages.add_section({ key = "plugins", title = "Plugins", pages = {} })
pages.add_page("plugins", {
	key = "plugins",
	title = "Plugins",
	icon = "plugins",
	build = function()
		return ui.column { w_full = true, widgets.toggle_row("plugins.safe_mode", "Safe mode", "Start with every plugin off.") }
	end,
})
```

`widgets.toggle_row`, `widgets.toggle` and `widgets.setting_radios` read `settings.values` and write through `settings.set`, so a key they use must exist in `bridge::SettingKey`; a page with state of its own publishes its own `<area>.<name>` values instead.

## User panel, status and profiles

`shell/user_panel.luau` is oxidcord's user panel: the 40-point user area (the 32-point avatar with its status dot, the name, and under it the custom status with its emoji, or the username), then the mute, deafen and settings buttons; the settings button opens settings on My Account (settings.open). Rust publishes `user.panel` (`{ name, subtitle, emoji, emoji_image, status, latency }`) from `bridge::PanelUser` in `src/ui/profiles.rs`; the area's tooltip is the gateway latency. Mute and deafen only toggle their icons until voice arrives. Pressing the user area calls `user.status_menu` with the pointer position.

Everything the panel and the profiles open overlaps the message area, so it is drawn in Rust by `src/profile/` (`ProfileLayer`), a layer above the timeline slot in the root layout. The layer takes the whole window only while something is open; a press outside the open surface closes it, and Escape closes it from the root.

- Status menu (`status_menu.rs`): 260 points wide, its bottom-left corner 20 points above the press and kept 8 points inside the window, as oxidcord's. The header, Edit Profile (settings on Profiles), Online, Idle, Do Not Disturb and Invisible (`Command::SetStatus`), the custom status entry, Switch Accounts (logs until 41) and Log Out (`login.sign_out`'s path). The session saves the status in the settings proto (`docs/rest.md`).
- Custom status dialog (`custom_status.rs`): oxidcord's form, the emoji button (the emoji picker, Unicode only), the text (128 characters), the clear-after chips (Today, 4 hours, 1 hour, 30 minutes, Don't clear), and Clear Status, Cancel and Save, which send `Command::SetCustomStatus`.
- Profile popout (`popout.rs`): 300 points wide, the banner (or accent colour), the 80-point avatar with status, badges, name, handle and pronouns, the custom status, then activities, About Me, Member Since, roles, the mutual counts, the actions (Message, Add Friend, Request Sent or Accept, Block, Copy ID) and View Full Profile.
- Full profile (`full.rs`): oxidcord's 600-point card on the `overlay` scrim below the title bar, with the User Info, Mutual Servers and Mutual Friends tabs.

`bridge::ProfileCard` is the projection both read: the user, the member, the cached profile (`Command::LoadProfile`, cached by the session for five minutes, a missing profile shows what is known), presence, the custom status and `bridge::activity_line` for each activity (Playing, Listening to, Watching, Streaming, Competing in, Spotify's song, artist and album). Your own card reads the status from the settings proto instead of presence. The layer recomputes the card on every change while it is open and redraws only when it differs.

Every profile entry point goes through `Link::open_profile(user_id, at, anchor)`: avatars, author names and mentions in the timeline (`TimelineView::on_profile`, anchored top left at the pointer) and member rows (`Link::open_member_profile`, anchored top right). Message opens the existing DM or sends `Command::OpenDm` and opens it once it arrives.

## Shared values and host actions

Names are `<area>.<name>` in snake case, with the area the region or service that owns them. Plugins (45) will get `plugin.<id>.<name>`.

| Kind | Names |
|---|---|
| Shared values (Rust publishes, Luau reads) | `login.screen`, `login.acknowledged`, `login.checking`, `login.message`, `login.error`, `login.unsaved`, `session.status`, `session.storage_warning`, `rail.items`, `rail.home_mentions`, `rail.open_folders`, `sidebar.rows`, `sidebar.header`, `sidebar.guild_id`, `sidebar.guild_name`, `timeline.channel_id`, `timeline.channel_name`, `timeline.channel_topic`, `dms.rows`, `dms.requests`, `home.page`, `composer.open`, `composer.placeholder`, `composer.replace`, `composer.focus`, `composer.reply`, `composer.editing`, `composer.files`, `composer.suggestions`, `composer.selected`, `composer.typing`, `composer.notice`, `composer.drop_hover`, `composer.limit`, `members.heights`, `members.window`, `members.jump`, `user.avatar`, `user.avatar_ready`, `user.panel`, `window.maximized`, `settings.page`, `settings.values`, `settings.account`, `settings.notes`, `settings.about` |
| Actions (`ui.action`, fire and forget) | `login.acknowledge`, `login.submit`, `login.continue`, `login.sign_out`, `rail.select_guild`, `rail.select_home`, `rail.toggle_folder`, `rail.drop`, `rail.add_server`, `rail.discover`, `sidebar.select_channel`, `sidebar.toggle_category`, `sidebar.mark_channel_read`, `sidebar.mark_guild_read`, `sidebar.copy_channel_link`, `sidebar.invite`, `sidebar.server_settings`, `sidebar.create_channel`, `sidebar.notification_settings`, `sidebar.mute_guild`, `sidebar.mute_channel`, `sidebar.edit_channel`, `sidebar.delete_channel`, `sidebar.leave_guild`, `dms.select`, `dms.close`, `dms.search`, `dms.create_group`, `home.show_page`, `window.minimize`, `window.maximize`, `window.close`, `media.copy_image`, `composer.choose`, `composer.add_files`, `composer.remove_file`, `composer.start_reply`, `composer.start_edit`, `composer.cancel_reply`, `composer.toggle_ping`, `composer.cancel_edit`, `composer.confirm`, `composer.retry`, `composer.discard`, `composer.dismiss`, `members.search`, `members.search_remote`, `members.open_profile`, `settings.open`, `settings.close`, `settings.set`, `settings.open_data_folder`, `settings.open_web`, `settings.test_notification`, `user.status_menu` |
| Functions (`ui.call_sync`) | `media.visible`, `media.hover`, `media.reduced_motion`, `frame.slot`, `composer.changed`, `composer.key`, `members.scroll` |

Rust registers them in `Link::install` (and `Integration::install` for desktop integration). Luau declares each shared value once in `state.luau` and wraps each action in `actions.luau`, so a rename touches one Luau file. State only the interface cares about stays in Luau: `ui.state` in the module that owns it, or in `state.luau` (`state.shell.members_visible`) when several modules share it. Never publish the whole store. Publish small projections from `bridge` and let zgx reconcile them.

## Window frame

The window is frameless (`Titlebar::Hidden`): Windows keeps its shadow, rounded corners and resize edges, and the title bar is kyoko's own, as oxidcord's is. The drag region is `window_drag = true`: a double click maximizes and a right click opens the system menu. The buttons call `window.minimize`, `window.maximize` and `window.close`. `frame.rs` finds the native window to read whether it is maximized (`window.maximized` swaps the maximize and restore icons). Windows 11 snap layouts on the maximize button need zgx's caption hit testing, which only `Titlebar::Inset` has, so they are missing for now.

## Adding a screen

1. Add the module under `shell/` (or `components/` if it is reusable), styled only from `theme.luau`, with icons by name.
2. Publish what it needs from Rust as `<area>.<name>` shared values, declare them in `state.luau`, and add its actions to `actions.luau`.
3. Add its capture steps (`docs/parity.md`) and compare `pair.png` with oxidcord.
