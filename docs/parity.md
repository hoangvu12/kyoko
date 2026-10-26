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

`KYOKO_CAPTURE_PLUGINS=<id>,<id>` in the environment of `cargo xtask parity capture` turns those bundled plugins on in kyoko before any screen's steps (approved in-process, as the Allow button does), and kyoko signals ready only once the display hooks have answered (`Display::pending`). Use it with `--out` to keep a plugin-on run apart: `KYOKO_CAPTURE_PLUGINS=show-me-your-name` with `timeline,member-list,profile-popout` shows ShowMeYourName's usernames (ticket 48). Oxidcord ignores it.

The mock fixtures are deterministic for a seed and fixture date, but some text is relative to the wall clock, such as "Today" and "Yesterday" in timestamps. Captures of the same day compare cleanly. Both apps show timestamps in the machine's local time zone.

## Adding kyoko's steps for a screen

A UI ticket that builds a screen also adds its capture steps:

1. Add the screen name to `Screen::named` in `apps/desktop/src/capture.rs`, as a new `Screen` variant when it needs its own steps.
2. Drive the window there through the same `Command`s and actions the UI uses, for example selecting a guild, opening a popout or switching a settings page. Never reach into state directly to fake a screen.
3. Extend `Link::settled` in `apps/desktop/src/ui.rs` with whatever the screen waits on, such as images or a REST reply.
4. Run `cargo xtask parity capture --screens <name>` and compare `pair.png`.

Oxidcord's steps are already in the reference patch. When the parity list grows, add the row here and the steps to the patch, then regenerate it as described in `tools/parity/README.md`.

## Comparing with the Midnight mockups

Kyoko's look is moving to Discord's layout in the Midnight theme (`docs/design/midnight.md`). A restyled screen is checked against its page in `docs/design/mockups/` instead of oxidcord:

```
cargo xtask parity mockup --screens timeline --port 8496
cargo xtask parity mockup
cargo xtask parity list
```

For each screen it captures kyoko against mockcord exactly as `parity capture` does, then renders the screen's mockup in a headless Chrome or Edge and writes the pair. With no `--screens` it runs every screen in the table below. `parity list` shows each screen's mockup page.

- The browser is `--browser <exe>`, else the `KYOKO_PARITY_BROWSER` environment variable, else Chrome or Edge under Program Files, Program Files (x86) or the user's local app data, else the `App Paths` registry entries for `chrome.exe` and `msedge.exe`. The command stops before building anything when it finds none. It starts one browser for the run with a throwaway profile and closes it at the end.
- The mockup is opened as a file, unchanged, in a 1280 by 800 viewport over the Chrome DevTools Protocol. The screenshot is taken after the page's load event, once `document.fonts.ready` has resolved, every image has decoded and two frames have drawn. If a stylesheet, an image or the Inter font did not load, the mockup side fails with the missing URLs in `<screen>/mockup.log`; it is never captured half loaded.
- The mockups load Inter from Google Fonts and Twemoji from jsDelivr, so a run needs the network: each run uses a fresh browser profile and fetches them again.
- Scale: the mockup is rendered with a device scale factor equal to kyoko's window scale, the width of kyoko's capture divided by 1280 (the system scale when kyoko failed). At 150% both images are 1920 by 1200 and line up pixel for pixel. Unlike `parity capture`, this mode does not assume 100% display scale and does not warn about it; it warns when kyoko's client area is not 1280 by 800 at its own scale, and when the two images still differ in size, in which case there is no `diff.png`.
- The output goes to `target/parity-mockup/` (`--out` changes it) with the same layout as the oxidcord mode: `<screen>/mockup.png`, `kyoko.png`, `pair.png` (mockup left, kyoko right), `diff.png` (changed pixels in red over a dimmed mockup), the kyoko and mockcord logs, `browser.log` and `index.html` with the differing-pixel share. `--seed`, `--anchor`, `--port`, `--no-build` work as in `parity capture`. The command fails if any capture failed.

Mockup text, avatars and icons are placeholders, so the share never reaches zero; read `pair.png` and `diff.png` for layout, sizes, spacing and colour.

### Screens and their mockups

The table is mirrored in `MOCKUPS` in `tools/xtask/src/parity/mockup.rs`; change both together. A screen is listed only when kyoko has capture steps for it and a mockup shows the same state.

| Screen | Mockup |
|---|---|
| `app-frame` | `midnight-discord.html` |
| `login-tos` | `login.html` |
| `login-token` | `login-token.html` |
| `login-qr-scanned` | `login-qr-scanned.html` |
| `login-qr-expired` | `login-qr-expired.html` |
| `login-web` | `login-web.html` |
| `login-email` | `login-email.html` |
| `login-2fa` | `login-2fa.html` |
| `login-captcha` | `login-captcha.html` |
| `login-unsaved` | `login-unsaved.html` |
| `server-rail` | `midnight-discord.html` |
| `server-rail-folder` | `midnight-discord.html` |
| `channel-sidebar` | `midnight-discord.html` |
| `dm-sidebar` | `dm.html` |
| `dm-group` | `group-dm.html` |
| `timeline` | `midnight-discord.html` |
| `composer` | `composer-markdown.html` |
| `composer-reply` | `composer-markdown.html` |
| `message-hover` | `midnight-discord.html` |
| `message-context-menu` | `message-menu.html` |
| `member-list` | `midnight-discord.html` |
| `user-panel` | `midnight-discord.html` |
| `status-menu` | `status-menu.html` |
| `custom-status` | `dialog-custom-status.html` |
| `profile-popout` | `profile-popout.html` |
| `full-profile` | `profile-full.html` |
| `quick-switcher` | `quick-switcher.html` |
| `search` | `search-results.html` |
| `lightbox` | `lightbox.html` |
| `emoji-picker` | `emoji-picker.html` |
| `gif-picker` | `gif-picker.html` |
| `sticker-picker` | `sticker-picker.html` |
| `threads-list` | `threads-list.html` |
| `thread-panel` | `threads-split-view.html` |
| `thread-create` | `threads-create.html` |
| `pins` | `pins.html` |
| `forum` | `forum.html` |
| `forum-post` | `forum-post.html` |
| `forum-post-create` | `forum-new-post.html` |
| `friends-online` | `home-friends.html` |
| `friends-all` | `friends-all.html` |
| `friends-pending` | `friends-pending.html` |
| `friends-blocked` | `friends-blocked.html` |
| `friends-add` | `friends-add-friend.html` |
| `add-server` | `dialog-add-a-server.html` |
| `forward` | `dialog-forward.html` |
| `guild-notifications` | `dialog-notification-settings.html` |
| `server-settings-overview` | `server-settings-overview.html` |
| `server-settings-roles` | `server-settings-roles.html` |
| `server-settings-emoji` | `server-settings-emoji.html` |
| `server-settings-stickers` | `server-settings-stickers.html` |
| `server-settings-members` | `server-settings-members.html` |
| `server-settings-invites` | `server-settings-invites.html` |
| `server-settings-bans` | `server-settings-bans.html` |
| `server-settings-audit-log` | `server-settings-audit-log.html` |
| `moderation-nickname` | `dialog-change-nickname.html` |
| `moderation-timeout` | `dialog-timeout.html` |
| `moderation-kick` | `dialog-kick.html` |
| `moderation-ban` | `dialog-ban.html` |
| `settings-my-account` | `settings-my-account.html` |
| `settings-appearance` | `settings-appearance.html` |
| `settings-notifications` | `settings-notifications.html` |
| `settings-plugins` | `settings-plugins.html` |
| `toast` | `toast-rate-limited.html` |
| `connection-offline` | `connection-lost.html` |
| `expressions-in-messages` | `expressions-in-messages.html` |
| `expressions-in-messages-emoji-popout` | `expressions-in-messages-emoji-popout.html` |
| `expressions-in-messages-sticker-popout` | `expressions-in-messages-sticker-popout.html` |
| `link-preview-while-typing` | `link-preview-while-typing.html` |

Several screens share `midnight-discord.html`, the main screen, which also shows the hover bar on a message; compare the region the screen is about. `composer` and `composer-reply` share `composer-markdown.html`, which has a draft and a reply bar.

Kyoko screens with no mockup yet: `channel-sidebar-collapsed`, `composer-silent-typing`, `member-list-scrolled`, `account-switcher`, `timeline-media` and the `timeline-*` showcase screens, and the settings pages other than My Account, Appearance, Notifications and Plugins.

Mockup pages with no kyoko screen yet, for the tickets that build them to map: `app-modal`, `app-modal-error`, `apps-launcher`, `built-in-gif`, `built-in-sticker`, `built-in-thread`, `command-attachment-option`, `command-attachment-option-picked`, `command-choices-and-autocomplete`, `command-choices-failed`, `command-choices-invalid`, `command-choices-loading`, `command-options`, `command-options-optional`, `command-options-remind`, `command-picker`, `command-picker-built-in`, `command-response-states`, `composer-drop`, `composer-markdown-over-limit`, `composer-plus-menu`, `composer-upload-failed`, `composer-uploads`, `delete-message-dialog`, `emoji-autocomplete`, `emoji-picker-no-nitro`, `emoji-picker-search`, `gif-picker-results`, `inbox`, `message-components`, `message-components-multi-select`, `message-components-v2`, `message-menu-own`, `poll-create`, `poll-in-messages`, `poll-in-messages-ended`, `reactions`, `reactions-list`, `settings-themes`, `voice-message`.

To map a new screen: give it kyoko capture steps (above) and a row in the Screens tables below, add the pair to `MOCKUPS` and to this table, and take the page off the unmapped list. Several kyoko screens may share a mockup, and a mockup with no matching kyoko state stays unmapped rather than paired with something close.

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
| `login-token` | 21 | As `login-tos`, then acknowledge the notice. Kyoko then switches to "Sign in with a token" and submits a token mockcord rejects, so the form shows "That token was rejected." as `login-token.html` does; its QR column shows a live code from mockcord's remote-auth gateway (ticket 42). Oxidcord's side only acknowledges the notice and its QR column shows its failed state, because it has no mock remote-auth gateway. | yes |

### 42 QR login

Mockup only: oxidcord has no mock remote-auth gateway, so `parity capture` skips its side of these screens (`MOCKUP_ONLY` in `tools/xtask/src/parity.rs`). In capture mode kyoko adds a `phone` query to the remote-auth URL, and mockcord's gateway plays the phone accordingly.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `login-qr-scanned` | 42 | As `login-tos`, then acknowledge the notice; the phone scans the code at once (`phone=scan`), so the column shows the scanning user's avatar and name and asks to confirm on the phone. Settles once the avatar has loaded. | yes |
| `login-qr-expired` | 42 | As `login-tos`, then acknowledge the notice; the phone lets the code expire at once (`phone=expire`, close 4003), so the column shows the dimmed code and Refresh. | yes |

### 43 Web login

Mockup only, like the QR screens: oxidcord's web login opens Discord itself, which a capture never loads.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `login-web` | 43 | As `login-tos`, acknowledge the notice, then "Sign in on discord.com": the web login window over the login screen, its page being mockcord's `/login` (the allowed origin under mock). Settles once the page has loaded and the QR code is drawn. | yes |

### 44 Email and password login

Mockup only: oxidcord has no email form. Kyoko drives the form in-process with mockcord's accounts (`docs/testing.md`); the typed text lives in Luau, so the fields show empty where the mockups show an address and a code.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `login-email` | 44 | As `login-tos`, acknowledge the notice, then log in as `mock@kyoko.invalid` with a wrong password: "Login or password is invalid." under the password field. Settles once the QR code is drawn. | yes |
| `login-2fa` | 44 | As `login-tos`, acknowledge the notice, then log in as `mfa@kyoko.invalid`, and send the code `000000`, which mockcord answers with a rate limit: the narrow two-factor card with the "Discord refused the sign-in for now..." line. | yes |
| `login-captcha` | 44 | As `login-tos`, acknowledge the notice, then log in as `captcha@kyoko.invalid`, which mockcord answers with a captcha: the hand-over to discord.com beside the QR code. | yes |
| `login-unsaved` | 44 | Opens the store on an in-memory keyring that refuses writes, with plain-text storage off; acknowledge the notice and log in as `mock@kyoko.invalid`: the narrow "Your token couldn't be saved" card. | yes |

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
| `account-switcher` | 41 | `status-menu`, then Switch Accounts: the accounts view with the signed-in account checked, Add an Account and Manage Accounts. | yes |
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
| `lightbox` | 32 | Default selection, then open the newest loaded image attachment in the lightbox. | yes |
| `emoji-picker` | 32 | Default selection, then open the composer's picker on the emoji tab at (1001, 755). | yes |
| `gif-picker` | 32 | As `emoji-picker`, on the GIF tab, once the trending GIFs have loaded. | yes |
| `sticker-picker` | 32 | As `emoji-picker`, on the sticker tab, once the sticker packs have loaded. | yes |

### 33 Threads and forums

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `threads-list` | 33 | Default selection, then open the channel's thread list from the top bar at (871, 46). | yes |
| `thread-panel` | 33 | Default selection, then open the channel's first thread beside it and wait for its history. | yes |
| `thread-create` | 33 | Default selection, then open the create-thread dialog for the channel. | yes |
| `pins` | 33 | Default selection, then open the pinned messages from the top bar at (958, 46). | yes |
| `forum` | 33 | Open the first guild's forum channel. | yes |
| `forum-post` | 33 | As `forum`, then open its first post and wait for its history. | yes |
| `forum-post-create` | 33 | As `forum`, then open the new-post dialog. | yes |

### 34 Friends

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `friends-online` | 34 | Select Home, then open Friends on the Online tab. | yes |
| `friends-all` | 34 | As `friends-online`, on the All tab. | yes |
| `friends-pending` | 34 | As `friends-online`, on the Pending tab. | yes |
| `friends-blocked` | 34 | As `friends-online`, on the Blocked tab. | yes |
| `friends-add` | 34 | As `friends-online`, on the Add Friend tab. | yes |

Oxidcord's steps wait 2 seconds after the DM opens before showing Friends, because its selection sync otherwise closes the page again.

### 35 Community flows

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `add-server` | 35 | Default selection, then open the add-a-server dialog. | yes |
| `forward` | 35 | Default selection, then open the forward dialog for the newest message. | yes |
| `guild-notifications` | 35 | Default selection, then open the guild's notification settings. | yes |

Oxidcord has no rules (membership screening) or onboarding screens, so they have no rows; kyoko draws them in the same dialog style (`docs/ui.md`, "Community dialogs"). The forward preview names mentions where oxidcord prints their raw ids, and kyoko's input placeholders are dimmer than oxidcord's, as in 31's screens.

### 36 Moderation and server settings

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `server-settings-overview` | 36 | Default selection, then open server settings on Overview. The mock fixtures seed three bans, their audit entries and two invites in the first guild. | yes |
| `server-settings-roles` | 36 | As above, on Roles. | yes |
| `server-settings-emoji` | 36 | As above, on Emoji. | yes |
| `server-settings-stickers` | 36 | As above, on Stickers. | yes |
| `server-settings-members` | 36 | As above, on Members. | yes |
| `server-settings-invites` | 36 | As above, on Invites. | yes |
| `server-settings-bans` | 36 | As above, on Bans. | yes |
| `server-settings-audit-log` | 36 | As above, on Audit Log. | yes |
| `moderation-nickname` | 36 | Default selection, then open the change-nickname dialog for the newest other author. | yes |
| `moderation-timeout` | 36 | As above, the timeout dialog. | yes |
| `moderation-kick` | 36 | As above, the kick dialog. | yes |
| `moderation-ban` | 36 | As above, the ban dialog. | yes |

### 37 Account and settings pages

The voice and video and updates pages are left out, as ticket 37 says; ticket 45 adds the Plugins page. Ticket 57 adds the Themes page through the settings page registry. Kyoko has no Developer page: its developer mode switch is on Advanced.

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
| `settings-themes` | 57 | As above, on Themes. Oxidcord shows Built In, an empty From Plugins and its catalog URL; kyoko shows Built In (with the two first party themes and each card's version, author and description), an empty Local Themes and the Online Themes links box. | yes |
| `settings-developer` | 37 | As above, on Developer. | no |

### 45 Plugin runtime

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `settings-plugins` | 45 | Default selection, then open settings on Plugins. Oxidcord lists no plugins on a fresh data directory; kyoko lists its bundled plugins (ClearURLs, OnePingPerDM, SilentTyping, TextReplace), disabled. | yes |

### 46 TextReplace

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `settings-text-replace` | 46 | Default selection. Kyoko sets three TextReplace rules (a plain rule, a Luau pattern rule and a rule with an "only if" condition) and turns TextReplace on through `PluginHost`, opens settings on Plugins, then presses TextReplace's Settings button through the window (`capture::TEXT_REPLACE_SETTINGS`). Oxidcord has no plugins and no such screen, so capture it with `--apps kyoko` and check it by eye. | yes |

### 47 SilentTyping

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `composer-silent-typing` | 47 | Default selection, then type a one-line draft into the composer, as `composer`. Kyoko first turns SilentTyping on (the Allow button's `PluginHost::approve`), waits for its button to mount in the composer, and clicks it twice through the window (off, then on again). Oxidcord has no SilentTyping and takes `composer`'s steps, so the share is the button. | yes |

### 53 ExpressionCloner

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `message-menu-clone` | 53 | With `KYOKO_CAPTURE_PLUGINS=expression-cloner`. Default selection; kyoko waits for the plugin's menu items, marks a custom emoji of the open server as under the pointer (`TimelineView::aim`, what a press on an inline emoji does) and right-clicks the last message as `message-context-menu`, so the menu ends with Clone Emoji. Oxidcord has no plugins: capture it with `--apps kyoko` and check it by eye. | yes |
| `clone-dialog` | 53 | With `KYOKO_CAPTURE_PLUGINS=expression-cloner`. Default selection, then the Clone Emoji dialog for a custom emoji of the open server (`Dialog::Clone`). Kyoko only, `--apps kyoko`. | yes |

### 52 MessageLogger

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `message-logger-deleted` | 52 | With `KYOKO_CAPTURE_PLUGINS=message-logger`. Default selection; kyoko waits for the plugin to turn deleted messages on, sends "spoiler alert: the ship sinks at the end", which mockcord's moderator deletes 400 ms later (`mockcord::MODERATED_PHRASE`), and waits for the row to show in red with "(deleted)" and "Remove from log". Kyoko only, `--apps kyoko`; no Midnight mockup, check it by eye. | yes |
| `message-logger-edits` | 52 | With `KYOKO_CAPTURE_PLUGINS=message-logger`. Default selection; kyoko sends "See you at 6" and edits it twice through `Command::EditMessage`, so the two earlier versions show dimmed above "See you at 7:30, sorry!". Kyoko only, `--apps kyoko`. | yes |

### Midnight 05 Timeline restyle

Kyoko only (`--apps kyoko`, or `parity mockup`): oxidcord has no such states. The messages are sent through the composer's own commands into the default channel; mockcord unfurls a `klipy.com` link as a GIF from the GIF provider and any other link as a link embed, as Discord does.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `expressions-in-messages` | 05 | Default selection, then send a Klipy GIF link, a PNG sticker of the server, a line with a custom emoji of the server and 🙏, and a jumbo line (💀, the custom emoji, 😭). Hover the GIF (its favourite star shows) through `Ui::dispatch`, then open the custom emoji's popout as a press on it does. | yes |
| `expressions-in-messages-emoji-popout` | 05 | As `expressions-in-messages`, then the popout of the jumbo 😭 (a default emoji). | yes |
| `expressions-in-messages-sticker-popout` | 05 | As `expressions-in-messages`, then the sticker's popout. | yes |
| `link-preview-while-typing` | 05 | Default selection, then send `docs are up https://github.com/refact0r/midnight-discord` and hover it at (640, 690): its link embed and the hover bar with Remove all embeds. | yes |

### 55 ShowHiddenChannels and ShowHiddenThings

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `hidden-channel` | 55 | With `KYOKO_CAPTURE_PLUGINS=show-hidden-channels`. Default selection, then the first server with a channel the user cannot view (mockcord's `staff-only`, hidden from everyone but Admin), whose row shows locked and dimmed; kyoko opens its details through `Link::open_hidden_channel`, what a click on the row does: name, kind, topic, last activity, and who is let in or kept out. Kyoko only, `--apps kyoko`; no Midnight mockup, check it by eye. | yes |
| `member-notes` | 55 | With `KYOKO_CAPTURE_PLUGINS=show-hidden-things`. As `member-list`; the fixture's timed-out member (also flagged by AutoMod) shows a red timer and a yellow shield after its name. Kyoko only, `--apps kyoko`. | yes |

### 38 Toasts, transitions and connection status

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `toast` | 38 | Default selection, then show an error toast with a rate-limit message. Kyoko pushes it through the toast layer, as a refused command would. | yes |
| `connection-offline` | 38 | Default selection, then mark the timeline disconnected. Oxidcord sets its timeline disconnected; kyoko pins the connection banner to its reconnecting state (`Link::drive_disconnected`). The real path, a drop, an offline period and the reconnect against mockcord, is covered by `crates/bridge/tests/bridge/connection.rs`. | yes |

Tickets 39 (translations) and 40 (tray and desktop integration) have no screens of their own. Translations are checked by running any row with another locale once ticket 39 adds a locale option to capture mode.
