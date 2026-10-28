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

`KYOKO_CAPTURE_PLUGINS=<id>,<id>` in the environment of `cargo xtask parity capture` turns those bundled plugins on in kyoko before any screen's steps (approved in-process, as the Allow button does, and waited on until a plugin with a UI half has it mounted), and kyoko signals ready only once the display hooks have answered (`Display::pending`). Use it with `--out` to keep a plugin-on run apart: `KYOKO_CAPTURE_PLUGINS=show-me-your-name` with `timeline,member-list,profile-popout` shows ShowMeYourName's usernames (ticket 48). Oxidcord ignores it.

`KYOKO_CAPTURE_THEMES=<file>,<file>` copies those theme files (`.css` or `.json`) into the capture's themes folder and turns them on, in order, before the screen settles. `KYOKO_CAPTURE_THEMES=crates/bridge/tests/bridge/fixtures/discord-variables.theme.css` with `--out` shows a theme that sets only Discord's variables over Midnight (Midnight 25). Oxidcord ignores it.

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
| `composer-markdown-over-limit` | `composer-markdown-over-limit.html` |
| `composer-uploads` | `composer-uploads.html` |
| `composer-drop` | `composer-drop.html` |
| `composer-upload-failed` | `composer-upload-failed.html` |
| `composer-plus-menu` | `composer-plus-menu.html` |
| `message-hover` | `midnight-discord.html` |
| `message-context-menu` | `message-menu.html` |
| `message-context-menu-own` | `message-menu-own.html` |
| `delete-message-dialog` | `delete-message-dialog.html` |
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
| `gif-picker-results` | `gif-picker-results.html` |
| `emoji-picker-search` | `emoji-picker-search.html` |
| `emoji-picker-no-nitro` | `emoji-picker-no-nitro.html` |
| `emoji-autocomplete` | `emoji-autocomplete.html` |
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
| `settings-themes` | `settings-themes.html` |
| `toast` | `toast-rate-limited.html` |
| `connection-offline` | `connection-lost.html` |
| `expressions-in-messages` | `expressions-in-messages.html` |
| `expressions-in-messages-emoji-popout` | `expressions-in-messages-emoji-popout.html` |
| `expressions-in-messages-sticker-popout` | `expressions-in-messages-sticker-popout.html` |
| `link-preview-while-typing` | `link-preview-while-typing.html` |
| `reactions` | `reactions.html` |
| `reactions-list` | `reactions-list.html` |
| `command-picker` | `command-picker.html` |
| `command-options` | `command-options.html` |
| `command-options-optional` | `command-options-optional.html` |
| `command-options-remind` | `command-options-remind.html` |
| `command-choices-invalid` | `command-choices-invalid.html` |
| `voice-message` | `voice-message.html` |
| `inbox` | `inbox.html` |
| `command-choices-and-autocomplete` | `command-choices-and-autocomplete.html` |
| `command-choices-loading` | `command-choices-loading.html` |
| `command-choices-failed` | `command-choices-failed.html` |
| `command-attachment-option` | `command-attachment-option.html` |
| `command-attachment-option-picked` | `command-attachment-option-picked.html` |
| `command-response-states` | `command-response-states.html` |
| `message-components` | `message-components.html` |
| `message-components-multi-select` | `message-components-multi-select.html` |
| `message-components-v2` | `message-components-v2.html` |
| `app-modal` | `app-modal.html` |
| `app-modal-error` | `app-modal-error.html` |
| `poll-create` | `poll-create.html` |
| `poll-in-messages` | `poll-in-messages.html` |
| `poll-in-messages-ended` | `poll-in-messages-ended.html` |
| `apps-launcher` | `apps-launcher.html` |
| `command-picker-built-in` | `command-picker-built-in.html` |
| `built-in-gif` | `built-in-gif.html` |
| `built-in-sticker` | `built-in-sticker.html` |
| `built-in-thread` | `built-in-thread.html` |

Several screens share `midnight-discord.html`, the main screen, which also shows the hover bar on a message; compare the region the screen is about. `composer` and `composer-reply` share `composer-markdown.html`, which has a draft and a reply bar.

Kyoko screens with no mockup yet: `channel-sidebar-collapsed`, `composer-silent-typing`, `member-list-scrolled`, `account-switcher`, `timeline-media` and the `timeline-*` showcase screens, and the settings pages other than My Account, Appearance, Notifications, Plugins and Themes.

Every mockup page now has a kyoko screen.

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
| `login-tos` | 21 | Start without a token on a fresh data directory: the login screen with the unacknowledged Terms of Service notice. Since v1 68 the left column is the "Sign in on discord.com" column, not the email form `login.html` shows, so this screen, `login-qr-scanned` and `login-qr-expired` differ from their mockups in the left column by design (`docs/sign-in.md`). | yes |
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
| `login-email` | 44 | As `login-tos`, acknowledge the notice, then "Use email and password instead" (v1 68) and log in as `mock@kyoko.invalid` with a wrong password: "Login or password is invalid." under the password field. Settles once the QR code is drawn. | yes |
| `login-2fa` | 44 | As `login-tos`, acknowledge the notice, then open the email form and log in as `mfa@kyoko.invalid`, and send the code `000000`, which mockcord answers with a rate limit: the narrow two-factor card with the "Discord refused the sign-in for now..." line. | yes |
| `login-captcha` | 44 | As `login-tos`, acknowledge the notice, then open the email form and log in as `captcha@kyoko.invalid`, which mockcord answers with a captcha: the hand-over to discord.com beside the QR code. | yes |
| `login-unsaved` | 44 | Opens the store on an in-memory keyring that refuses writes, with plain-text storage off; acknowledge the notice, open the email form and log in as `mock@kyoko.invalid`: the narrow "Your token couldn't be saved" card. | yes |

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
| `dm-sidebar` | 24 | Select Home, which opens the most recent DM, and wait for its history and the recipient's profile (the profile column, M21). | yes |
| `dm-group` | 24 | Open the first group DM from the DM list and wait for its history; the member column shows beside the chat (M21). | yes |

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
| `timeline-files` | 26 | As above, jump to `-# showcase: files and stickers`: file and video cards, APNG, GIF and Lottie stickers (kyoko draws the Lottie one; oxidcord draws nothing). | yes |
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
| `status-menu` | 30 | Default selection, then open the status menu from the user panel at (99, 774) and its status submenu (Midnight 20). | yes |
| `account-switcher` | 41 | `status-menu`, then Switch Accounts: the accounts submenu with the signed-in account checked, Add an Account and Manage Accounts. | yes |
| `custom-status` | 30 | Default selection, then open the custom status dialog. | yes |
| `profile-popout` | 30 | Default selection, then open the profile popout of the newest other author at (348, 404), anchored top left. | yes |
| `full-profile` | 30 | Default selection, then open the full profile of the newest other author. | yes |

### 31 Quick switcher and search

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `quick-switcher` | 31 | Default selection, then open the quick switcher and type "gen" (kyoko, as `quick-switcher.html` has; oxidcord opens it with no query). Kyoko's field shows its caret, being focused. | yes |
| `search` | 31 | Default selection, then search the guild for "the" and show the results panel. Kyoko runs it through the top bar's `search.run`. | yes |

### 32 Lightbox and pickers

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `lightbox` | 32 | Default selection, then open the newest loaded image attachment in the lightbox. | yes |
| `emoji-picker` | 32 | Default selection, then open the composer's picker on the emoji tab at (954, 755). | yes |
| `gif-picker` | 32 | As `emoji-picker`, on the GIF tab, once the trending GIFs have loaded. | yes |
| `sticker-picker` | 32 | As `emoji-picker`, on the sticker tab, once the sticker packs have loaded. | yes |

### 33 Threads and forums

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `threads-list` | 33 | Default selection, then open the channel's thread list from the top bar at (871, 46). | yes |
| `thread-panel` | 33 | Default selection, then open the channel's first thread beside it and wait for its history. | yes |
| `thread-create` | 33 | Default selection, then start a thread from the channel's newest message (kyoko: the New Thread side panel with its starter message; oxidcord: its create-thread dialog). | yes |
| `pins` | 33 | Default selection, then open the pinned messages from the top bar at (958, 46). | yes |
| `forum` | 33 | Open the first guild's forum channel. | yes |
| `forum-post` | 33 | As `forum`, then open its first post and wait for its history. | yes |
| `forum-post-create` | 33 | As `forum`, then open the new-post form (kyoko: inline at the top of the forum; oxidcord: its dialog). | yes |

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
| `server-settings-overview` | 36 | Default selection, then open server settings on Overview; since Midnight 23 they open as the settings dialog over the dimmed app. The mock fixtures seed three bans, their audit entries and two invites in the first guild. | yes |
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

The updates page is left out, as ticket 37 says; ticket 45 adds the Plugins page and 57 the Themes page. Midnight 19 turns settings into Discord's dialog: the Clips note page is gone, Voice & Video is a greyed page saying Kyoko has no voice calls, the safety locks, links and GIF usage moved to the Kyoko group's Safety & Privacy page, the data folder to Storage, the startup switches to System, and Advanced is titled Developer (the `advanced` key is kept). Kyoko's screens show the dialog over the dimmed main screen; oxidcord's are its full-screen settings.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `settings-my-account` | 37 | Default selection, then open settings on My Account. | yes |
| `settings-profiles` | 37 | As above, on Profiles. | yes |
| `settings-privacy` | 37 | As above, on Data & Privacy (with Activity Privacy). | yes |
| `settings-devices` | 37 | As above, on Devices. | yes |
| `settings-connections` | 37 | As above, on Connections. | yes |
| `settings-friend-requests` | 37 | As above, on Friend Requests. | yes |
| `settings-appearance` | 37 | As above, on Appearance. | yes |
| `settings-accessibility` | 37 | As above, on Accessibility. | yes |
| `settings-chat` | 37 | As above, on Chat. | yes |
| `settings-notifications` | 37 | As above, on Notifications. | yes |
| `settings-keybinds` | 37 | As above, on Keybinds. | yes |
| `settings-language` | 37 | As above, on Language & Time. | yes |
| `settings-streamer-mode` | 37 | As above, on Streamer Mode. | yes |
| `settings-advanced` | 37 | As above, on Developer (key `advanced`). | yes |
| `settings-safety` | M19 | As above, on Safety & Privacy. | yes |
| `settings-storage` | M19 | As above, on Storage. | yes |
| `settings-voice` | M19 | As above, on the greyed Voice & Video page. | yes |
| `settings-system` | M19 | As above, on System. | yes |
| `settings-themes` | 57 | As above, on Themes. Oxidcord shows Built In, an empty From Plugins and its catalog URL; kyoko shows its Local Themes tab (the notice on how themes are read, the folder card, Built In with each card's version, author and description, and an empty Local Themes); the links box is on the Online Themes tab. | yes |

### 45 Plugin runtime

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `settings-plugins` | 45 | Default selection, then open settings on Plugins. Oxidcord lists no plugins on a fresh data directory; kyoko lists its bundled plugins (ClearURLs, OnePingPerDM, SilentTyping, TextReplace), disabled. | yes |

### 46 TextReplace

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `settings-text-replace` | 46 | Default selection. Kyoko sets three TextReplace rules (a plain rule, a Luau pattern rule and a rule with an "only if" condition) and turns TextReplace on through `PluginHost`, opens settings on Plugins, then presses TextReplace's Settings button through the window (`capture::TEXT_REPLACE_SETTINGS`). Oxidcord has no plugins and no such screen, so capture it with `--apps kyoko` and check it by eye. | yes |
| `settings-plugin-permissions` | 84 | Default selection. Kyoko opens settings on Plugins and presses ClearURLs' switch through the window (`capture::CLEAR_URLS_SWITCH`); the plugin was never approved, so its permission prompt opens: Midnight's dialog (`bg_4`, `radius_lg`, title, body, footer band) on its own `overlay` scrim over the settings dialog. Kyoko only, `--apps kyoko`; no Midnight mockup, check it by eye. | yes |

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

### Midnight 08 Expression picker

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `gif-picker-results` | 08 | As `gif-picker`, then the Trending GIFs tile, once the trending list has loaded; the first GIF hovered shows its favourite star. | yes |
| `emoji-picker-search` | 08 | As `emoji-picker`, then type "sm" in the search field; the first result hovered. | yes |
| `emoji-picker-no-nitro` | 08 | As `emoji-picker`, against a mockcord started with `--no-nitro` (the parity tool passes it for this screen): the grid scrolled to the first locked section and its first locked emoji hovered, so the footer says why. | yes |

### Midnight 16 Composer restyle and extras

Kyoko only (`--apps kyoko`, or `parity mockup`). Files are handed to the main composer from memory (a 320 by 240 PNG, a 2 KB MP4 and a text file), as the file picker would; messages go through the session's own commands into the default channel.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `composer-markdown-over-limit` | 16 | Default selection, reply to the newest message from someone else, then a draft 12 characters over the message limit that ends with markdown (bold, italic, a quote, inline code, a spoiler): the field grown to half the chat and the counter at -12 in red. | yes |
| `composer-uploads` | 16 | Default selection, then attach `screenshot.png`, `clip.mp4` and `notes.txt`, spoiler the first, and move the pointer over the second card (`Ui::dispatch`) so its spoiler, edit and remove toolbar shows. | yes |
| `composer-drop` | 16 | Default selection, then files hovering the window (`Link::files_hovering`): the dashed "Upload to #channel" card. | yes |
| `composer-upload-failed` | 16 | Default selection, then send "here it is" with `stalled-upload.mp4` (mockcord never answers its upload) and "and the other angle" with `broken-upload.mp4` (mockcord answers its upload with a 500), and refuse a `cat.mov` twice the upload limit: an upload in progress, "Upload failed" with Retry and Remove, and the too-big notice above the composer. | yes |
| `composer-plus-menu` | 16 | Default selection, then the `+` menu opened above the composer as its button does (`Link::composer_plus`). | yes |

### Midnight 09 Emoji autocomplete

Kyoko only (`--apps kyoko`, or `parity mockup`). The parity tool starts mockcord with `--no-nitro --autocomplete-emoji`: the first guild gets `:smug:` and `:smol_wave:`, the second `:smolcat:` and an animated `:smash:`, so the second guild's pair is locked.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `emoji-autocomplete` | 09 | Default selection, then the draft `wow :sm` with the caret at its end: the suggestion container with the default emoji, the server's two custom emoji and the two locked ones last, the first row selected. | yes |

### Midnight 10 Command picker and options

Kyoko only (`--apps kyoko`, or `parity mockup`). Mockcord serves the mockups' apps (Mockbot, Remindy, Polly, Snapper) and their commands; for `command-picker` the parity tool adds `--frequent-commands`, which seeds the account's frecency settings with `/remind`, `/roll` and the built-in `/shrug`. Each screen asks for the command index, types `/` and waits for the picker.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `command-picker` | 10 | Default selection, then `/` in the field: the picker with Frequently Used, each app and Built-In, the first row selected. | yes |
| `command-options` | 10 | As `command-picker` without the seeded uses, then pick `/roll`: the app icon, `/roll` and its focused `sides` chip, the hint bar with "+1 optional". | yes |
| `command-options-optional` | 10 | As `command-options`, with `sides` set to 20 and the optional list opened (`count`). | yes |
| `command-options-remind` | 10 | As `command-options`, picking `/remind`: the focused `when` chip and the `message` chip. | yes |
| `command-choices-invalid` | 10 | As `command-options`, with `sides` set to 1000: the chip and the hint in red with "Must be a number between 1 and 100". | yes |
| `command-options-only-optional` | 75 | As `command-options`, picking Mockbot's `/lookup` (two optional string options): the OPTIONS list opens at once; then `query` clicked in it (`Ui::dispatch`): its focused chip right after `/lookup` and the "+1 option" hint after it. No mockup. | yes |

### Midnight 11 Command choices, autocomplete and file options

Kyoko only (`--apps kyoko`, or `parity mockup`). Steps as `command-options`, picking another command. Mockbot's `/echo` lists its six languages as choices; `/translate` asks Mockbot for them through autocomplete, and Snapper's `/resize` takes a file. For `command-choices-loading` the parity tool adds `--slow-autocomplete` (Mockbot answers after ten minutes), for `command-choices-failed` `--failing-autocomplete` (the interaction is refused with a 500).

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `command-choices-and-autocomplete` | 11 | Pick `/echo`, `text` set to hello, `language` focused: the LANGUAGE list above the hint bar with English selected. | yes |
| `command-choices-loading` | 11 | Pick `/translate`, `text` set to hello, `language` focused while Mockbot has not answered: "Loading options..." with the dots. | yes |
| `command-choices-failed` | 11 | As `command-choices-loading`, the autocomplete refused: "Couldn't load options" and Retry. | yes |
| `command-attachment-option` | 11 | Pick `/resize`: the focused `file` chip and the drop box above the hint bar. | yes |
| `command-attachment-option-picked` | 11 | As `command-attachment-option`, then a generated `cat.png` picked: the file chip with its thumbnail and name, `width` focused. | yes |

### Midnight 12 Command replies

Kyoko only (`--apps kyoko`, or `parity mockup`). The parity tool adds `--slow-followups` (Mockbot's deferred `/think` follows up after ten minutes) and `--hold-ping` (mockcord accepts `/ping` but sends no gateway event, so it stays "Sending command..."). The mockup's "This interaction failed" row needs message components (Midnight 13) and is not staged.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `command-response-states` | 12 | Default selection, then run `/think` (thinking), `/roll sides:20` (answered), `/whisper` (ephemeral, with "Only you can see this · Dismiss message"), `/ping` (sending) and `/broken` (failed: "The application did not respond", ephemeral), each once the previous one shows. | yes |

### Midnight 13 Message components and app modals

Kyoko only (`--apps kyoko`, or `parity mockup`). The component messages are Mockbot's in mockcord's `showcase: components` section of `#showcase`; the modal is Mockbot's `/bug`. For `message-components` the parity tool adds `--slow-followups` (Mockbot answers Refresh after ten minutes), for `app-modal-error` `--failing-modals` (Mockbot fails every modal submit).

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `message-components` | 13 | Open `#showcase` at the components section with "Pick an action" on top, press Refresh (spinner while Mockbot has not answered), choose Banana in the fruit select, then open it again: the list with Banana checked. | yes |
| `message-components-multi-select` | 13 | As `message-components` without pressing anything, Apple and Grapes picked in the "Which fruits do you like?" select with its list open. | yes |
| `message-components-v2` | 13 | Open `#showcase` with "did the release post go out" on top: Mockbot's Components V2 container below it (text, a section with a thumbnail, a separator, a three-image gallery and a section with the Changelog link button). | yes |
| `app-modal` | 13 | Default selection, run `/bug`, then type "Reaction tooltip wraps to three lines" into Title: the app modal over the window. | yes |
| `app-modal-error` | 13 | As `app-modal`, with What happened? filled in too and Submit pressed while Mockbot fails modals: "Something went wrong. Try again." above the footer. | yes |

### Midnight 14 Built-in commands

Kyoko only (`--apps kyoko`, or `parity mockup`). Steps as `command-options`, picking a built-in. Mockcord's GIF search lists the matching GIFs first, then the rest; its pack stickers `hello` and `bye` carry the tag `wave`.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `command-picker-built-in` | 14 | `/` in the field, then the rail's Built-In section and `/gif` selected: the Built-In rows with `/gif` and its `query` chip. | yes |
| `built-in-gif` | 14 | Pick `/gif` with `query` set to cat, once the search has answered: the strip of GIFs above the composer, the first selected. | yes |
| `built-in-sticker` | 14 | Pick `/sticker` with `query` set to wave, once the stickers have loaded: the strip of matching stickers, the first selected. | yes |
| `built-in-thread` | 14 | Pick `/thread` with `name` set to picker polish and `message` to notes from testing, `name` focused: the two chips and the hint bar. | yes |

### 55 ShowHiddenChannels and ShowHiddenThings

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `hidden-channel` | 55 | With `KYOKO_CAPTURE_PLUGINS=show-hidden-channels`. Default selection, then the first server with a channel the user cannot view (mockcord's `staff-only`, hidden from everyone but Admin), whose row shows locked and dimmed; kyoko opens its details through `Link::open_hidden_channel`, what a click on the row does: name, kind, topic, last activity, and who is let in or kept out. Kyoko only, `--apps kyoko`; no Midnight mockup, check it by eye. | yes |
| `member-notes` | 55 | With `KYOKO_CAPTURE_PLUGINS=show-hidden-things`. As `member-list`; the fixture's timed-out member (also flagged by AutoMod) shows a red timer and a yellow shield after its name. Kyoko only, `--apps kyoko`. | yes |

### Midnight 07 Menus, dialogs and toasts

Kyoko only (`--apps kyoko`, or `parity mockup`). The message is sent through the composer's own commands into the default channel.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `message-context-menu-own` | 07 | Default selection, then send "can someone ping me" and right-click it at (640, 690), as `message-context-menu` does: the menu of your own message. | yes |
| `delete-message-dialog` | 07 | As `message-context-menu-own`, but instead of the menu run Delete Message on that message (`TimelineView::run_action`), which opens the confirmation with its preview. | yes |

### 63 Friend action confirmations

Kyoko only (`--apps kyoko`); no Midnight mockup, the dialogs follow `delete-message-dialog`'s style, so check them by eye.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `friend-remove-dialog` | 63 | As `friends-online`, then ask to remove the first friend (`Link::ask_remove_friend`, what the More menu's Remove Friend does): the Remove Friend confirmation. | yes |
| `friend-block-dialog` | 63 | As `friend-remove-dialog`, but ask to block that friend (`Link::ask_block`, what the More menu's Block and the profile's Block do). | yes |

### Midnight 06 Reactions and super reactions

Kyoko only (`--apps kyoko`, or `parity mockup`). The messages are the `-# showcase: reactions` section at the end of mockcord's `#showcase` channel: normal, reacted, custom and super reactions (one of them yours), with no `burst_colors`, so super chips take Midnight's purple-to-blue gradient.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `reactions` | 06 | Open `#showcase` at its newest messages, then hover the 👍 chip of "midnight 2.1.1 is out…" through `Ui::dispatch`: the row's hover bar, the add-reaction and add-super-reaction buttons, and the chip's tooltip naming who reacted (loaded from the reactions route). | yes |
| `reactions-list` | 06 | As `reactions`, then open the Reactions list of that message on 🔥 (`TimelineView::open_reactions`, what View Reactions does) and hover its second reactor, which shows the remove button (the mock user has Manage Messages there). | yes |

### Midnight 18 Voice message and audio playback

Kyoko only (`--apps kyoko`, or `parity mockup`). The messages are the `-# showcase: voice messages` section of mockcord's `#showcase` channel, just before the reactions: two voice messages (Ogg Opus, 8 and 7 seconds, with a waveform) and an MP3 under its file card. In capture mode the player's output is a frozen clock (`Listening::freeze`, the manual sink), so nothing sounds and the position stays where the steps leave it.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `voice-message` | 18 | Open `#showcase` with the audio file message at the bottom of the timeline, then click the first voice message's play button and its waveform at half its width through `Ui::dispatch`: the first pill playing at 0:04 with half its waveform in `accent_2`, the second at rest showing 0:07, and the audio player under the file card at 0:00 / 1:24 (its length read through the media pipeline). | yes |

### Midnight 17 Poll creation and polls

Kyoko only (`--apps kyoko`, or `parity mockup`). The polls are the `-# showcase: polls` and `-# showcase: ended poll` sections of mockcord's `#showcase` channel, just before the reactions: "What should we play tonight?" with three answers carrying Unicode emoji and 7, 3 and 2 votes, open for another day; the same poll voted (Minecraft); and an ended copy with its poll result message.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `poll-create` | 17 | Default selection, then open Create Poll (`ComposerLayer::open_poll`, what the `+` menu's item does) and fill the question and four answers (two with emoji, the last empty) through `PollForm::fill`. | yes |
| `poll-in-messages` | 17 | Open `#showcase` with the voted poll at the bottom of the timeline: the unvoted poll above it with radios, Show results and a disabled Vote, the voted one with bars, the check and Remove Vote. | yes |
| `poll-in-messages-ended` | 17 | Open `#showcase` with the poll result message at the bottom: the ended poll with the winner's green bar and trophy and "Poll closed", then the result line with View Poll. | yes |

### Midnight 15 Apps launcher

Kyoko only (`--apps kyoko`, or `parity mockup`). For `apps-launcher` the parity tool adds `--more-apps`: mockcord's command index then also lists the mockup's Warden, Lyra, Ticketeer and Countr (one command each) and Arcade, an activity-only app with just a launch entry point, and the account's frecency settings are seeded with `/roll`, `/remind`, `/poll`, `/snap`, `/play` and `/warn`, so Recents read Mockbot, Remindy, Polly, Snapper, Lyra, Warden.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `apps-launcher` | 15 | Default selection, then open the launcher the way the composer's apps button does (`Link::use_apps`, anchored to the `composer` slot) and wait for the command index and the frecency settings: search, Recents tiles and the eight Apps in this Server rows (Arcade left out). | yes |

### 38 Toasts, transitions and connection status

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `toast` | 38 | Default selection, then show an error toast with a rate-limit message. Kyoko pushes it through the toast layer, as a refused command would. | yes |
| `connection-offline` | 38 | Default selection, then mark the timeline disconnected. Oxidcord sets its timeline disconnected; kyoko pins the connection banner to its reconnecting state (`Link::drive_disconnected`). The real path, a drop, an offline period and the reconnect against mockcord, is covered by `crates/bridge/tests/bridge/connection.rs`. | yes |

Tickets 39 (translations) and 40 (tray and desktop integration) have no screens of their own. Translations are checked by running any row with another locale once ticket 39 adds a locale option to capture mode.

### 62 Moderation menu follow-ups

Kyoko only (`--apps kyoko`): oxidcord's member menu has no Roles submenu to compare. No Midnight mockup, check it by eye.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `moderation-roles-submenu` | 62 | As `member-list`, then right-click, through `Ui::dispatch`, the name of the first member whose roles the current user can change (at the window's right edge), and hover the menu's Roles item: the submenu opens to the left of the member menu, inside the window, with a checked box on the roles the member holds. | yes |

### 95 Channel settings

Kyoko only (`--apps kyoko`): oxidcord has no channel settings page. No Midnight mockup; compare by eye with Discord's (Midnight's server settings look, Discord's fields). Mockcord's first server is boost tier 1, so a voice channel's bitrate goes up to 128 kbps; its regions come from mockcord's `GET /guilds/{id}/regions`.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `channel-settings-overview` | 95 | Default selection, then open channel settings for the open text channel (`Link::drive_channel_settings`): name, topic, slowmode, Age-Restricted Channel, Announcement Channel, Hide After Inactivity and Category. | yes |
| `channel-settings-voice` | 95 | Default selection, then open channel settings for the server's first voice channel: name, slowmode, Content Visibility, Bitrate (8 to 128 kbps), Video Quality, User Limit, Region Override, once the regions have loaded. | yes |
| `channel-settings-permissions` | 95 | As `channel-settings-overview`, on Permissions: the roles and members with overwrites, @everyone first. | yes |
| `channel-settings-invites` | 95 | As `channel-settings-overview`, on Invites, once `GET /channels/{id}/invites` has answered. | yes |

### Midnight 27 Inbox

Kyoko only (`--apps kyoko`, or `parity mockup`): oxidcord has no inbox. The mentions are mockcord's recent mentions of the mock user (`GET /users/@me/mentions`), so the channels, authors and texts differ from the mockup's.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `inbox` | 27 | Default selection, then open the inbox from the channel header's inbox button at (1137, 37) (`Link::toggle_inbox`): the Mentions tab, fetched when it opens, with the newest mentions grouped by channel under their server, once every avatar and server icon has loaded. | yes |

### 54 ReadAllNotificationsButton

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `server-rail-read-all` | 54 | With `KYOKO_CAPTURE_PLUGINS=read-all-notifications`. As `server-rail`; kyoko waits for the plugin's UI half to mount, so the Read All Notifications tile shows under Home. Oxidcord has no plugins: capture it with `--apps kyoko` and check it by eye; `server-rail` without the variable shows the rail unchanged. | yes |

### 49 PlatformIndicators and ForceOwnerCrown

Kyoko only (`--apps kyoko`): oxidcord has no plugins. No Midnight mockup, check them by eye. With both plugins off, `member-list` and `timeline` stay as they were: no icons beside names and no crown (mockcord's servers show their Admin role apart, which hides the owner's crown as Discord does).

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `name-decorations` | 49 | With `KYOKO_CAPTURE_PLUGINS=platform-indicators,force-owner-crown`. As `member-list`: each author in the timeline and each member shows a monitor, phone or globe per platform in their status colour (mockcord spreads people over desktop, mobile, both, and web), and the owner shows the yellow crown after their name. | yes |
| `name-decorations-profile` | 49 | With `KYOKO_CAPTURE_PLUGINS=platform-indicators,force-owner-crown`. As `profile-popout`: the author's platform icons come first in the badge row. | yes |

### 56 GameActivityToggle

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `user-panel-game-activity` | 56 | With `KYOKO_CAPTURE_PLUGINS=game-activity-toggle`. Default selection; kyoko waits for the plugin's controller button to mount in the user panel and clicks it at (194, 760) through the window, so sharing turns off: the crossed-out controller in red and the "Game activity is hidden" toast. Kyoko only, `--apps kyoko`; no Midnight mockup, check it by eye. | yes |

### 81 Server profiles

Kyoko only (`--apps kyoko`). Mockcord gives every sixth member of a server (from the third) a server avatar, every twelfth an animated one, and every eighth (from the third) an animated server banner; those members' profiles carry a server bio ("<username>, server edition") in `guild_member_profile`. Every fifth pool user has an animated global avatar and every fourth a global banner.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `server-profile` | 81 | Default selection, then open the profile popout of the open server's member with a server banner (one with a server avatar first) at (348, 404), anchored top left: the server banner, the server avatar, the server nick and the server bio. | yes |

### 92 Profiles like Discord

Kyoko only (`--apps kyoko`); compare by eye with Discord. Mockcord gives a user a profile theme when the timestamp part of its id (in seconds from the fixture epoch) is even and a profile effect when it is a multiple of four; a member with a server look has a server theme of its own.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `profile-menu` | 92 | Default selection, then open the profile menu of the newest other author at (348, 404), as the DM column's More button does: View Full Profile, Share Profile, Invite to Server (when there is a server to invite to), Ignore, Block, Report User Profile, Copy User ID. | yes |

### 82 Image grid

Kyoko only (`--apps kyoko`): oxidcord shows each image alone. No Midnight mockup has image attachments; compare by eye with Discord and the rules in `docs/ui.md` ("Image grid"). The sections open `#showcase`, before `-# showcase: markdown`, so the sections oxidcord reaches without paging back stay where they were (`IMAGE_GRID_SECTIONS` in mockcord's showcase fixture); mockcord serves every image at 400 by 300, so tiles show crops of its gradient.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `timeline-image-grid` | 82 | As `timeline-markdown`, jump to `-# showcase: image grid`: two 11:1 screenshots stacked at full width, then a wide and a tall image side by side. | yes |
| `timeline-image-grid-three` | 82 | As above, jump to `-# showcase: image grid of three`: a large square and two stacked, then five images (2+3). | yes |
| `timeline-image-grid-four` | 82 | As above, jump to `-# showcase: image grid of four`: four mixed shapes in a 2 by 2 grid. | yes |
| `timeline-image-grid-ten` | 82 | As above, jump to `-# showcase: image grid of ten`: one full-width tile, then rows of three. | yes |

### 98 Voice join, panel, mute and deafen

Kyoko only (`--apps kyoko`); compare by eye with Discord. The app's mock voice runs with manual audio, so no device is opened.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `voice-panel` | 98 | Default selection, then join the open server's first voice channel (Lounge) through `voice.join` and wait for Voice Connected: the voice panel between the channel sidebar and the user panel, Lounge / server name, Disconnect. | yes |

### 83 Server rail hover card

Kyoko only (`--apps kyoko`): no Midnight mockup has the rail's hover card; compare by eye with Discord and "Server rail" in `docs/ui.md`. Mockcord puts two or three people in each server's Lounge and Gaming, so every server tile carries the speaker badge.

| Screen | Ticket | Steps | Kyoko |
|---|---|---|---|
| `server-rail-hover` | 83 | Default selection, then point at the first server in the open mock folder at (45, 155) and wait for its hover card: the name, the speaker and the voice avatars. | yes |
