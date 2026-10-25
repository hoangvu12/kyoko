# Midnight: kyoko's target look

Kyoko's interface moves from a 1:1 port of oxidcord to Discord's own desktop layout, styled with refact0r's Midnight theme in its Vencord flavour (MIT, `github.com/refact0r/midnight-discord`, version 2.1.1). This page is the design spec: every value here is read from `themes/flavors/midnight-vencord.theme.css` and the `build/midnight.css` it imports, or from Discord's own layout where Midnight leaves it alone. The reference screenshot is `docs/design/reference/midnight-vencord.png`.

Kyoko cannot run BetterDiscord or Vencord themes: they are CSS selectors against Discord's DOM, and kyoko has no DOM. What it can read is a theme's variable block (`--bg-4`, `--accent-2`, `--gap`, ...). Midnight is built so that nearly all of its look follows from those variables, so the plan is: kyoko's built-in look is Midnight's structure, and its colours and sizes come from a Midnight-style variable block, which a user can swap.

## Palette

Midnight names its colours by role. Resolved values of the Vencord flavour:

| Variable | Value | Role |
|---|---|---|
| `--text-0` | `#282828` (= `--bg-4`) | text on coloured elements |
| `--text-1` | `#f1e9da` | bright text on coloured elements |
| `--text-2` | `#e4d2b4` | headings, author names, important text |
| `--text-3` | `#d4be98` | normal text, hovered icons |
| `--text-4` | `#a79a83` | icon buttons, channel names, offline |
| `--text-5` | `#d4be9866` | muted channels, timestamps, placeholders |
| `--bg-1` | `#433f3d` | dark buttons pressed, scrollbar thumb |
| `--bg-2` | `#3c3836` | dark buttons, secondary buttons |
| `--bg-3` | `#31302f` | the window behind the panels, code blocks, inputs, popouts |
| `--bg-4` | `#282828` | every panel's background, modals |
| `--hover` | `#6965631a` | list items and buttons hovered; light borders |
| `--active` | `#69656333` | selected and pressed; panel borders |
| `--active-2` | `#6965634c` | extra state for transparent buttons |
| `--message-hover` | `#0000001a` | message row hovered |
| `--accent-1` | `#db9aab` (purple-1) | links, mention text, brand text |
| `--accent-2` | `#d3869b` (purple-2) | small accents: mention bars, badges, bot tag, switches, unread pill |
| `--accent-3` | `#d3869b` (purple-3) | accent buttons, the new-messages bar, text selection |
| `--accent-4` | `#cd748c` (purple-4) | accent buttons hovered |
| `--accent-5` | `#c6627c` (purple-5) | accent buttons pressed |
| `--accent-new` | `--accent-2` | what Discord draws red: mention badges, unread divider, muted mic |
| `--online` | `#a8b665` | status |
| `--idle` | `#d8a656` | status |
| `--dnd` | `#ea6962` | status |
| `--streaming` | `#d3869b` | status |
| `--offline` | `#a79a83` | status |
| `--button-border` | `#ffffff1a` | border of coloured buttons |

Base scales (each has 1 to 5, light to dark): red `#ed7e78 #ea6962 #ea6962 #e7534b #e33e35`, green `#b1be74 #a8b665 #a8b665 #9ead52 ...`, blue `#8ab7ad #7caea3 #7caea3 ...`, yellow `#ddb169 #d8a656 #d8a656 ...`, purple as the accent column above. Danger buttons use red-3, red-4 hovered, red-5 pressed.

Derived backgrounds:

- Mentioned message: a gradient left to right, `--accent-2` at 10% opacity up to 40% of the width, fading to transparent; 5% when hovered. A 4-point `--accent-2` bar sits at its left edge.
- Reply to you: the same gradient in `--text-3`, bar in `--text-2`.
- Ephemeral messages: the mention gradient.
- Mention chip: `--accent-2` at 10%, text `--accent-1`.
- Reacted reaction: `--accent-2` at 20% (30% hovered), border `--accent-2`, text `--text-1`. Unreacted: `--hover` with a `--hover` border, `--active` hovered, text `--text-3`.
- Poll: voted fill `--accent-2` at 10%, winner `--green-2` at 10%.
- Feedback backgrounds (critical, info, positive, warning): the base colour at 20%.
- Scrim behind dialogs: black at 70%, lightbox black at 90%.

## Type

- Interface font: Inter (the flavour's choice; Midnight's base uses Figtree), then `gg sans`, `Noto Sans`, `Helvetica Neue`, Arial, sans-serif. Regular text weight 400. Inter is under the SIL Open Font License and can be bundled.
- Code: `gg mono`, `Source Code Pro`, Consolas, Monaco, monospace.
- Sizes are Discord's: message text 16, author name 16 semibold, timestamp 12, channel names 16 medium, category headers 12 semibold uppercase in the classic layout (Discord's 2025 layout and the reference use 14 medium, not uppercase, with a trailing chevron), member names 16 medium, member activity 12.

## Structure: floating panels

The look is panels floating on a darker frame:

- The window background is `--bg-3`. Every region is a panel on it: `--bg-4` background, `--radius-lg` corners, a 1-point `--active` border (`--border-thickness`), and the border stays `--active` on hover (the hover transition is 0.2 s ease).
- Panels are separated by `--gap`, 12 points, and inset 12 from the window edges on the left, right and bottom.
- The panels: server rail, channel or DM sidebar, user panel, channel header, chat (timeline and composer together), member list. Popout profiles, search results, the thread panel, friends columns and settings are panels too.
- Title bar: `--top-bar-height` is `--gap`, 12 points, a thin draggable strip above the panels. The title is hidden; the inbox and help buttons move into the channel header's right end. Window controls are three 14-point circles, each in a 28-point wide hit box: minimize `--idle`, maximize `--online`, close `--dnd`; brightness 70% on hover, scale 0.9 pressed. They sit at the top right, outside the panels, padded `gap - 7 + 1` from the edge.

Left column, top to bottom: the server rail panel and the channel sidebar panel side by side with a 12-point gap, then the user panel. With `--small-user-panel: on` (the flavour's setting) the user panel spans only the channel sidebar's width, and the rail runs the full height.

### Server rail

- Width: avatar size + 12 padding on each side + 2 border, so 74 at Discord's 48-point server icons (Discord's 2025 layout offers 40; the reference uses 48).
- 12 points of padding above and below the list inside the panel.
- The selected/unread marker at the left edge is 4 points wide with 4-point rounded ends, centred in the 12-point padding.
- Home button: Midnight replaces the Discord logo with a custom SVG (the flavour's pixel-art mark; Midnight's base uses a crescent moon) masked to 65% of the button in `--text-4`, white when hovered or selected, rotating -360 degrees and scaling to 0.8 over 0.4 s on hover. Kyoko should use its own mark here the same way.
- Folders, the mention pill, unread pill and add-server button keep Discord's shapes, in palette colours: mention pill `--accent-new`, add-server icon `--text-3` (`--bg-4` hovered).

### Channel sidebar

- The guild header has no background of its own (the panel shows through); the server name with a chevron, and an action button at the right (invite in the reference).
- Channel rows: icon `--text-4`, `--text-3` when hovered or unread; name `--text-4`, brighter when unread. Selected row background `--active`, hovered `--hover`, rounded.
- List hover animation (`--animations: on`): a hovered channel row slides right by 10 points over 0.2 s ease. The unread marker is an 8-point dot (not Discord's bar) that sits half outside the left edge (-4) and slides to 4 on hover; threads -8 to 0.
- "1 New" style labels in `--accent-2`.

### User panel

- Its own panel, 12 points under the channel sidebar. Avatar with status, name and status line, then the mic, headphone and settings buttons. Mute and deafen when active use `--accent-new` with a 10% (20% hovered) wash instead of Discord's red.

### Channel header

- Its own panel above the chat, 12-point gap below it. Height: avatar size + 8 + 2 border, so 58 at 48.
- Left: channel icon, name in `--text-2`, a dot, the topic in `--text-4`. Right: threads, notifications, pins, member list, the search field (`--bg-3`, rounded, placeholder `--text-5`), then inbox and help, each a 32-point button with a 20-point icon, `--text-4`, `--text-3` hovered.

### Chat panel

- The timeline and composer share one panel.
- New messages bar: a floating bar inset 12 from the panel's top and sides, `--accent-3`, small radius, "N new messages since 5:11 PM" left and "Mark As Read" with its icon right, text `--text-0`.
- Messages have `--radius-md` corners and a 4-point left inset, so hover and highlight backgrounds read as rounded cards. Hover is `--message-hover`.
- Highlight bars (mention, reply, ephemeral) are 4 points wide, fully rounded, inset by `--radius-sm` top and bottom.
- Unread divider: 4 points thick, rounded, `--accent-new`.
- Embeds: `--radius-md`, a 4-point left border in the embed colour (`--active` without one) and 1-point `--active` borders on the other sides.
- Code blocks, images and thread links: `--radius-md`. Code block background `--bg-3`.
- Reactions and the add-reaction button: 2-point borders.
- Text selection: `--accent-3` background, `--text-0` text.
- Composer: `--radius-md`, height 47 (`--chatbar-height`, to line up with the user panel), background `--active`, placeholder `--text-5`; icons `--text-4`. The send button's active icon is `--accent-2`. Midnight's `--custom-chatbar: separated` (composer in its own panel under the timeline) is off in this flavour.
- Typing line: names in `--text-3`.

### Member list

- Its own panel, 12 points right of the chat, as wide as its content needs.
- Group headers in `--text-4`: "the almighty — 2". Rows: 32-point avatar with status, name in the role colour, then the activity line. Hovered rows slide right 18 points over 0.2 s ease (DM rows too).
- Nameplates (Discord's collectible row backgrounds) fill the row with `--radius-lg` corners.
- Bot and app tags: `--accent-2` background, `--text-0` text.

## Other surfaces

- Context menus, popouts, the emoji and GIF pickers, autocomplete, tooltips and the inbox use the floating background `--bg-3`; menus have 8-point corners. Elevation shadows are off (`--elevation-*: none`).
- Profiles: outer corners `--radius-lg`, inner `--radius-lg - 4`, 1-point `--active` border.
- Modals and their footers: `--bg-4`.
- Inputs: `--bg-3`, border `--hover`, `--active` hovered, `--accent-2` focused, red-2 on error with red-2 at 10% behind.
- Switches: off `--bg-2` with an `--active` border; on `--accent-2`; white thumb. Checkboxes and radios: `--bg-2`, selected `--accent-2`.
- Primary buttons `--accent-3`/`4`/`5` with `--text-0` text and a `--button-border` border; secondary `--bg-2`, `--bg-1` hovered, text `--text-3`.
- Progress bars and sliders: track `--bg-2`, fill `--accent-2`, grabber `--text-3`.
- Notice banners: 40 high, `--radius-md`, inset 12 like a panel.
- Settings: the sidebar items `--bg-3` hovered and `--bg-2` selected, sliding right 10 points on hover.

## Motion

With `--animations: on`: list items (channels, members, DMs, settings tabs) slide on hover over 0.2 s ease; panel borders change over 0.2 s ease; the home icon spins over 0.4 s ease. Kyoko keeps these behind the reduced-motion setting.

## Sizes Discord sets and Midnight keeps

Discord's scale: radius xs 4, sm 8, md 12, lg 16; space xs 8, sm 12, md 16, lg 24. These are from Discord's 2025 client and should be checked against the reference before they become tokens.

## Not part of stock Discord

In the reference screenshot: the Spotify player above the user panel (Vencord's SpotifyControls plugin), the online and member counts at the top of the member list (a Vencord member-count plugin) and the translate button in the composer (Vencord's Translate plugin). Everything else in it is stock Discord: guild tags (COZY, NURO), role icons, avatar decorations, nameplates and display-name styles (fonts and gradients on names).
