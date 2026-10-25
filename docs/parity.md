# UI parity captures

Kyoko's interface is a 1:1 port of oxidcord 0.2.0-beta.1. A screen ticket is done when kyoko's capture of each of its screens matches oxidcord's side by side, apart from the documented text-rendering differences. This page lists every screen and state to compare and how both apps get there. `cargo xtask parity` reads the tables below, so a row here is a row in the capture run.

## Running a capture

```
tools/parity/oxidcord-ref.ps1
cargo xtask parity capture
cargo xtask parity capture --screens timeline,member-list --apps oxidcord
cargo xtask parity list
```

1. `tools/parity/oxidcord-ref.ps1` recreates the reference build. It clones oxidcord next to the kyoko checkout as `oxidcord-ref`, checks out `b852253` (release 0.2.0-beta.1), applies `tools/parity/oxidcord-ref.patch` and builds `oxidcord.exe` in debug. Run it once, and again whenever the patch changes. Pass `-Path` for another location and `-Force` to replace an existing checkout. The reference checkout is never committed.
2. `cargo xtask parity capture` builds `kyoko` and `mockcord`, then captures every listed screen from both apps. For each screen and each app it:
   - starts the standalone `mockcord` with `--seed 1 --port 8485 --anchor 2026-10-01T12:00:00Z`, so both apps load the same fixtures from one server;
   - starts the app in capture mode against it, with a fresh data directory;
   - waits for the app to signal that the screen has settled;
   - finds the app's largest visible window by process id and captures its client area, with the app's other visible windows drawn over it at their places (zgx shows menus, popovers and tooltips in popup windows of their own on Windows);
   - closes the app with `WM_CLOSE`, then kills it after 3 seconds, because zgx sometimes hangs on exit;
   - stops mockcord, so the next app starts on untouched fixtures (read states, drafts and acks included).
3. The output goes to `target/parity/`:
   - `<screen>/oxidcord.png` and `<screen>/kyoko.png`
   - `<screen>/pair.png`, the two side by side, labelled
   - `<screen>/diff.png` when both images exist at the same size: changed pixels in red over a dimmed oxidcord
   - `<screen>/<app>.log` and `<screen>/<app>-mockcord.log`
   - `index.html`, a table of every screen with its status and differing-pixel share, then every pair

Options: `--screens a,b` picks screens, `--apps oxidcord` or `--apps kyoko` captures one side, `--oxidcord <exe>` points at another reference build, `--seed`, `--anchor`, `--port`, `--out`, and `--no-build`. The command fails if any capture failed. Captures run on demand only, never in CI. The differing-pixel share is a hint for review, not a gate.

## Capture mode

Both apps take the same hidden flags:

- `--mock-server <url>` connects to a mockcord that is already running instead of starting one in-process. Kyoko uses REST v9 and oxidcord v10; mockcord serves both. The token is mockcord's `mock-token`. Oxidcord needs `--mock` as well.
- `--capture-screen <name>` drives the window to the named screen through the app's own navigation, then waits 2 seconds (oxidcord) or 1.5 seconds (kyoko) for it to settle.
- `--capture-ready <path>` is the file the app writes once the screen has settled.
- Both windows are 1280 by 800 logical pixels with oxidcord's default dark theme. Captures assume a display scale of 100%. The command warns when a window reports another DPI or client size.

Kyoko's capture mode lives in `apps/desktop/src/capture.rs`. It is only active when `--capture-screen` is passed, and it refuses to run without `--mock` or `--mock-server`. For a screen it has no steps for, kyoko exits with code 3 before opening a window, and the capture command draws a "not built yet" placeholder in its place. Oxidcord's capture mode is `crates/ui/src/shell/capture.rs` in the reference patch. When a step fails, oxidcord exits with code 4 and the pair shows "capture failed".

The mock fixtures are deterministic for a seed and fixture date, but some text is relative to the wall clock, such as "Today" and "Yesterday" in timestamps. Captures of the same day compare cleanly. Both apps show timestamps in the machine's local time zone.

## Adding kyoko's steps for a screen

A UI ticket that builds a screen also adds its capture steps:

1. Add the screen name to `Screen::named` in `apps/desktop/src/capture.rs`, as a new `Screen` variant when it needs its own steps.
2. Drive the window there through the same `Command`s and actions the UI uses, for example selecting a guild, opening a popout or switching a settings page. Never reach into state directly to fake a screen.
3. Extend `Link::settled` in `apps/desktop/src/ui.rs` with whatever the screen waits on, such as images or a REST reply.
4. Run `cargo xtask parity capture --screens <name>` and compare `pair.png`.

Oxidcord's steps are already in the reference patch. When the parity list grows, add the row here and the steps to the patch, then regenerate it as described in `tools/parity/README.md`.

## Screens

Unless a row says otherwise, both apps start signed in on the first guild in rail order with its first readable text channel open, which is oxidcord's default selection, and wait for that channel's history. "At (x, y)" is the window position oxidcord's steps use for a popout anchor or a synthetic mouse event. The kyoko column says whether kyoko has steps for the screen yet.

### 20 Theme, fonts, icons and app frame

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `app-frame` | 20 | Default selection: title bar, top bar and the frame around the panels. | yes |

### 21 Token login, ToS notice and sign-out

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `login-tos` | 21 | Start without a token on a fresh data directory: the login screen with the unacknowledged Terms of Service notice. | yes |
| `login-token` | 21 | As `login-tos`, then acknowledge the notice. The QR column shows its failed state, because the mock has no remote-auth gateway. Kyoko's QR column says QR login is coming soon (ticket 42). | yes |

### 22 Server rail and folders

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `server-rail` | 22 | Default selection. | yes |
| `server-rail-folder` | 22 | Default selection, then open every guild folder in the rail. | yes |

### 23 Channel sidebar

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `channel-sidebar` | 23 | Default selection. | yes |
| `channel-sidebar-collapsed` | 23 | Default selection, then collapse the guild's first category with `ToggleCategory`. | yes |

### 24 DM sidebar

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `dm-sidebar` | 24 | Select Home, which opens the most recent DM, and wait for its history. | yes |
| `dm-group` | 24 | Open the first group DM from the DM list and wait for its history. Kyoko has no group bar or group member panel yet. | yes |

### 25 Timeline view

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `timeline` | 25 | Default selection, scrolled to the newest message. | yes |

### 26 Message rendering

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `timeline-media` | 26 | Default selection, then jump to the newest loaded message with an image attachment. | yes |
| `timeline-markdown` | 26 | Open `#showcase` (the mock fixtures' channel with every kind of message), then jump to its `-# showcase: markdown` message: styles, headings, subtext, lists, a quote, a highlighted code block. | yes |
| `timeline-inline` | 26 | As above, jump to `-# showcase: inline`: spoilers, links, mentions, timestamps, custom and jumbo emoji. | yes |
| `timeline-embeds` | 26 | As above, jump to `-# showcase: embeds`: a rich embed with fields, an article and a link preview. | yes |
| `timeline-embeds-media` | 26 | As above, jump to `-# showcase: embed media`: video, image and GIF embeds. | yes |
| `timeline-attachments` | 26 | As above, jump to `-# showcase: attachments`: image attachments and a GIF. | yes |
| `timeline-files` | 26 | As above, jump to `-# showcase: files and stickers`: file and video cards, PNG, GIF and Lottie stickers. | yes |
| `timeline-replies` | 26 | As above, jump to `-# showcase: replies, polls and reactions`: reply previews (one to a missing original), a poll, reactions and an edited message. | yes |
| `timeline-system` | 26 | As above, jump to `-# showcase: system messages`: the unread divider and system messages with their icons. | yes |

### 27 Composer

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `composer` | 27 | Default selection, then type a one-line draft into the composer. | yes |
| `composer-reply` | 27 | Default selection, then reply to the newest message from someone else. | yes |

### 28 Message actions and context menus

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `message-hover` | 28 | Default selection, then move the mouse over the newest message at (640, 690) to show its action bar. Kyoko sends the move through zgx's `Ui::dispatch`. | yes |
| `message-context-menu` | 28 | As `message-hover`, then right-click there. | yes |

### 29 Member list

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `member-list` | 29 | Default selection; the member list shows at 1280 pixels wide. | yes |
| `member-list-scrolled` | 29 | As `member-list`, then scroll the member list to its end, with a wheel event at (1160, 450) in oxidcord. | yes |

### 30 User panel, status and profiles

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `user-panel` | 30 | Default selection. | yes |
| `status-menu` | 30 | Default selection, then open the status menu from the user panel at (99, 774). | yes |
| `custom-status` | 30 | Default selection, then open the custom status dialog. | yes |
| `profile-popout` | 30 | Default selection, then open the profile popout of the newest other author at (348, 404), anchored top left. | yes |
| `full-profile` | 30 | Default selection, then open the full profile of the newest other author. | yes |

### 31 Quick switcher and search

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `quick-switcher` | 31 | Default selection, then open the quick switcher with no query. Kyoko's field shows its caret, being focused. | yes |
| `search` | 31 | Default selection, then search the guild for "the" and show the results panel. Kyoko runs it through the top bar's `search.run`. | yes |

### 32 Lightbox and pickers

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `lightbox` | 32 | Default selection, then open the newest loaded image attachment in the lightbox. | no |
| `emoji-picker` | 32 | Default selection, then open the composer's picker on the emoji tab at (1001, 755). | no |
| `gif-picker` | 32 | As `emoji-picker`, on the GIF tab. | no |
| `sticker-picker` | 32 | As `emoji-picker`, on the sticker tab. | no |

### 33 Threads and forums

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `threads-list` | 33 | Default selection, then open the channel's thread list from the top bar at (871, 46). | no |
| `thread-panel` | 33 | Default selection, then open the channel's first thread beside it and wait for its history. | no |
| `thread-create` | 33 | Default selection, then open the create-thread dialog for the channel. | no |
| `pins` | 33 | Default selection, then open the pinned messages from the top bar at (958, 46). | no |
| `forum` | 33 | Open the first guild's forum channel. | no |
| `forum-post` | 33 | As `forum`, then open its first post and wait for its history. | no |
| `forum-post-create` | 33 | As `forum`, then open the new-post dialog. | no |

### 34 Friends

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `friends-online` | 34 | Select Home, then open Friends on the Online tab. | no |
| `friends-all` | 34 | As `friends-online`, on the All tab. | no |
| `friends-pending` | 34 | As `friends-online`, on the Pending tab. | no |
| `friends-blocked` | 34 | As `friends-online`, on the Blocked tab. | no |
| `friends-add` | 34 | As `friends-online`, on the Add Friend tab. | no |

### 35 Community flows

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `add-server` | 35 | Default selection, then open the add-a-server dialog. | no |
| `forward` | 35 | Default selection, then open the forward dialog for the newest message. | no |
| `guild-notifications` | 35 | Default selection, then open the guild's notification settings. | no |

### 36 Moderation and server settings

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `server-settings-overview` | 36 | Default selection, then open server settings on Overview. | no |
| `server-settings-roles` | 36 | As above, on Roles. | no |
| `server-settings-emoji` | 36 | As above, on Emoji. | no |
| `server-settings-stickers` | 36 | As above, on Stickers. | no |
| `server-settings-members` | 36 | As above, on Members. | no |
| `server-settings-invites` | 36 | As above, on Invites. | no |
| `server-settings-bans` | 36 | As above, on Bans. | no |
| `server-settings-audit-log` | 36 | As above, on Audit Log. | no |
| `moderation-nickname` | 36 | Default selection, then open the change-nickname dialog for the newest other author. | no |
| `moderation-timeout` | 36 | As above, the timeout dialog. | no |
| `moderation-kick` | 36 | As above, the kick dialog. | no |
| `moderation-ban` | 36 | As above, the ban dialog. | no |

### 37 Account and settings pages

The voice and video, plugins and updates pages are left out, as ticket 37 says. Kyoko has no Themes page until ticket 57 adds one through the settings page registry, and no Developer page: its developer mode switch is on Advanced.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `settings-my-account` | 37 | Default selection, then open settings on My Account. | yes |
| `settings-profiles` | 37 | As above, on Profiles. | yes |
| `settings-privacy` | 37 | As above, on Privacy and Safety. | yes |
| `settings-devices` | 37 | As above, on Devices. | yes |
| `settings-connections` | 37 | As above, on Connections. | yes |
| `settings-clips` | 37 | As above, on Clips. | yes |
| `settings-friend-requests` | 37 | As above, on Friend Requests. | yes |
| `settings-appearance` | 37 | As above, on Appearance. | yes |
| `settings-accessibility` | 37 | As above, on Accessibility. | yes |
| `settings-chat` | 37 | As above, on Chat. | yes |
| `settings-notifications` | 37 | As above, on Notifications. | yes |
| `settings-keybinds` | 37 | As above, on Keybinds. | yes |
| `settings-language` | 37 | As above, on Language. | yes |
| `settings-streamer-mode` | 37 | As above, on Streamer Mode. | yes |
| `settings-advanced` | 37 | As above, on Advanced. | yes |
| `settings-themes` | 37 | As above, on Themes. | no |
| `settings-developer` | 37 | As above, on Developer. | no |

### 38 Toasts, transitions and connection status

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `toast` | 38 | Default selection, then show an error toast with a rate-limit message. | no |
| `connection-offline` | 38 | Default selection, then mark the timeline disconnected. | no |

Tickets 39 (translations) and 40 (tray and desktop integration) have no screens of their own. Translations are checked by running any row with another locale once ticket 39 adds a locale option to capture mode.
