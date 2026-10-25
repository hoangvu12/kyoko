# kyoko v1: a lightweight native Discord client on zgx, forked from oxidcord

Status: ready-for-agent

## Problem Statement

I use Discord all day.

- The official desktop client is an Electron app that idles at around 2 GB of memory and a noticeable share of a CPU core.
- The usual way to customise it is Vencord, but that runs inside the same Electron client, either patched or as Vesktop, so it is just as heavy.
- Native alternatives are either terminal UIs (concord) or early projects. oxidcord is the closest match: a native Rust client with a good interface. However:
  - Its UI is GPUI Rust code, so it can't be scripted or hot-reloaded.
  - Its core has protocol and account-safety gaps.
  - It has no tests and no evidence of being run against real Discord.
  - Its plugin system (WebAssembly) is not the scriptable, Vencord-style system I want.

What I want:

- A Discord client that feels like the real one and looks exactly like oxidcord.
- It uses a fraction of the memory.
- It runs natively on Windows (the only platform for v1).
- It is careful with my account.
- It lets me run the Vencord plugins I actually use, as native plugins written in an easy scripting language.

## Solution

**kyoko** is a public GitHub fork of oxidcord, credited as such.

**Core.** It keeps oxidcord's headless Discord crates and fixes them:

- protocol models
- gateway
- REST
- state
- encrypted store
- login
- media
- markdown
- OS integrations

It replaces their session layer with a smaller one, adds a real account-safety layer, and fixes their protocol and fingerprint gaps.

**UI.** The UI is rebuilt on zgx, a GPU-rendered Rust UI framework whose views are written in Luau scripts:

- It is a 1:1 port of oxidcord's current UI.
- Every screen is verified by comparing screenshots of both apps running against the same mock Discord server.
- The message timeline, image decoding and emoji are Rust pieces made to look identical.

**Login.** Users sign in four ways:

- pasting a token
- scanning a QR code with the phone app
- signing in on Discord's own login page inside a webview
- a native email/password form with 2FA, which hands over to the webview when Discord demands a captcha

**Plugins.** A Luau plugin system modelled on Vencord's hook surface lets the chosen Vencord plugins run as native plugins:

- The plugins are permissioned and sandboxed.
- They may never automate actions or fake client features.
- Vencord features that patch Discord's own UI become built-in behaviour or settings instead.

## User Stories

### Fork, credit and project

1. As the kyoko maintainer, I want kyoko to be a real GitHub fork of oxidcord, so that anyone can see where it came from and the original author is credited.
2. As the maintainer, I want oxidcord's licence files, copyright line and NOTICE kept intact, so that kyoko honours the MIT/Apache terms.
3. As the maintainer, I want the README to credit oxidcord with a link and explain what kyoko changes, so that users understand the relationship.
4. As the maintainer, I want every user-visible name, crate name, binary, data directory, app id and icon renamed from oxidcord to kyoko, so that the two apps can be installed side by side without collisions and kyoko doesn't use oxidcord's identity.
5. As the maintainer, I want oxidcord's update-signing key, plugin-catalog signing key and GitHub update endpoints removed or replaced, so that kyoko never trusts or contacts infrastructure it doesn't own.
6. As the maintainer, I want the origin and licence of oxidcord's bundled emoji table established and attributed, so that kyoko ships no unattributed third-party data.
7. As the maintainer, I want the oxidcord repository kept as an upstream remote, so that I can review and hand-port useful upstream fixes later.
8. As a contributor, I want the contributor rules to say that behaviour tests at the session boundary are required, so that the "no unit tests" rule inherited from oxidcord no longer applies.
9. As a contributor, I want the research notes on zgx, the Discord protocol, the oxidcord review and the Vencord triage to live in the repo, so that I can understand every decision without the original conversation.

### Spike (first milestone)

10. As the maintainer, I want a short Windows spike that proves the risky parts end to end before the main work starts:
    - zgx builds with its required patches
    - the forked core logs in to real Discord with a throwaway token from an environment variable
    - a Luau server/channel list updates live through the event bridge
    - new messages appear live
    - one WebP avatar renders

    That way the plan is validated cheaply.
11. As the maintainer, I want the spike's findings written down, so that later tickets can adjust if an assumption fails.

### Login and accounts

12. As a user, I want to paste my Discord token to sign in, so that I have a fast, low-risk way in.
13. As a user, I want to sign in by scanning a QR code with the Discord mobile app, so that I never type my password into a third-party app.
14. As a user, I want the QR screen to show the scanning user's avatar and name, and ask me to confirm on my phone, so that I know the right account is being linked.
15. As a user, I want to sign in on Discord's real login page inside a kyoko window, so that captchas, every 2FA method, security keys and passkeys work exactly as on discord.com.
16. As a user, I want the web login window to leave no browsing data behind after signing in, so that my session isn't kept in a stray browser profile.
17. As a user, I want to sign in with email and password in a native form, so that I don't need a browser window when Discord doesn't ask for a captcha.
18. As a user, I want to complete TOTP, SMS or backup-code 2FA in the native form, so that 2FA accounts can use it.
19. As a user, when Discord demands a captcha during native login, I want kyoko to move me to the web login page instead of failing, so that I can finish signing in.
20. As a user, I want my session token stored in the Windows Credential Manager, so that it never sits in a plaintext file.
21. As a user, I want kyoko to refuse to store my token in plaintext unless I explicitly opt in, and to warn me when it does, so that a keyring failure never silently downgrades my security.
22. As a user, I want to sign out and have my token, cached messages and kept deleted messages wiped, so that nothing of my account is left behind.
23. As a user, I want to keep several accounts and switch between them, as oxidcord supports, so that I can use my alts.
24. As a user, I want kyoko to stop all requests and send me back to login when Discord rejects my token, so that a revoked session doesn't keep hammering Discord.
25. As a user, I want kyoko to pick up a token Discord refreshes during connection, so that my stored session doesn't go stale.
26. As a user, I want the login screen to tell me plainly that third-party clients can violate Discord's terms and risk my account, so that I decide with full knowledge.

### Connection and sync

27. As a user, I want kyoko to connect quickly and show my servers and DMs as soon as Discord's initial data arrives, so that startup feels instant.
28. As a user, I want kyoko to resume after brief network drops without reloading everything, so that I don't miss messages or lose my place.
29. As a user, I want kyoko to re-identify cleanly when resuming is impossible and to catch up on the open channel's history, so that no messages are missing after a long disconnect.
30. As a user, I want kyoko to stop retrying and tell me why when Discord closes the connection for a fatal reason, so that it never loops forever.
31. As a user, I want reconnect attempts to back off, so that a Discord outage doesn't make kyoko hammer the gateway.
32. As a user, I want kyoko to remember enough between launches that Discord sends a smaller initial payload, so that cold starts are faster and lighter.
33. As a user, I want DMs and group DMs that Discord delivers after the initial payload to appear, so that no conversation is missing.
34. As a user, I want a connection-status indicator, so that I know whether I'm live, reconnecting or offline.

### Servers, channels and navigation

35. As a user, I want a server rail with folders, unread and mention indicators, matching oxidcord's, so that I can see where activity is.
36. As a user, I want to reorder servers and manage folders, so that my rail stays organised.
37. As a user, I want a channel sidebar with categories, collapsed state, unread and mention badges and muted styling, so that navigation works like Discord's.
38. As a user, I want a DM sidebar with my conversations ordered by recent activity, so that I can find recent chats quickly.
39. As a user, I want a quick switcher to jump to any server, channel or DM by name, so that I can move around without the mouse.
40. As a user, I want threads and forum posts listed and openable, so that I can follow threaded conversations.
41. As a user, I want a friends view with online, all, pending and blocked lists, plus adding friends, so that I can manage relationships.
42. As a user, I want my user panel showing my avatar, status and custom status, with controls to change them, so that I control how I appear.
43. As a user, I want to view profiles in a popout and full profile, so that I can see who someone is.
44. As a user, I want to search messages in a server or DM with filters, so that I can find old messages.
45. As a server moderator, I want the moderation and server-settings screens oxidcord has, so that I can manage my servers from kyoko.

### Reading messages

46. As a user, I want the message timeline to stick to the newest message while I'm at the bottom, so that new messages are always visible.
47. As a user, I want my scroll position to stay put when older history loads above me, so that I don't lose my place while reading back.
48. As a user, I want older history to load automatically as I scroll up, so that I can read a channel's past seamlessly.
49. As a user, I want grouped messages, reply previews, edited markers, timestamps, system messages and an unread divider, rendered as oxidcord renders them, so that reading feels familiar.
50. As a user, I want Discord markdown rendered fully, so that messages look as their authors intended:
    - bold, italics, underline, strikethrough
    - headings, subtext, lists, quotes
    - inline code and highlighted code blocks
    - spoilers, links, masked links
    - user, role and channel mentions, custom emoji and timestamps
51. As a user, I want image and video attachments, embeds, stickers, polls and reactions shown inline, so that I don't need to leave the client.
52. As a user, I want animated GIFs, animated WebP, animated emoji and animated avatars to play, so that kyoko shows what everyone else sees.
53. As a user, I want to open images in a lightbox at full resolution, so that I can see details.
54. As a user, I want to see who is typing, so that I know a reply is coming.
55. As a user, I want to select and copy message text, so that I can quote or reuse it.
56. As a user, I want a reply whose original message isn't loaded to show the original anyway, fetched politely in the background, so that I never see "message could not be loaded".
57. As a user, I want mentions of users kyoko hasn't seen yet to resolve to their names, fetched politely, so that I never see "@unknown-user".

### Sending and acting on messages

58. As a user, I want a composer with multiline input, IME support, spellcheck, emoji autocomplete and mention autocomplete, so that writing feels like Discord.
59. As a user, I want to reply (with or without a ping), edit and delete my messages, so that I can manage my conversation.
60. As a user, I want to react with unicode and custom emoji through an emoji picker, so that I can respond quickly.
61. As a user, I want to upload files by picking, pasting or dropping them, with previews before sending, so that sharing is easy.
62. As a user, I want to run slash commands, so that bots work for me.
63. As a user, I want a failed send to show me why it failed (rate limit, permission, captcha, network) and not retry blindly, so that I never trigger account blocks by accident.
64. As a user, I want message context menus with copy text, copy link, copy ID, reply, edit, delete, pin and mark-unread, matching oxidcord's, so that every action is one right-click away.
65. As a user, I want to multi-select and delete my own messages without kyoko calling endpoints user accounts can't use, so that the feature never trips Discord's defences.

### Members and presence

66. As a user, I want a member list grouped by role and status that loads further members as I scroll, so that I can browse large servers fully, not just the first 100.
67. As a user, I want presence, status and activity shown on members and DMs, so that I know who's around.

### Notifications and read state

68. As a user, I want OS notifications for mentions and DMs, following my per-server and per-channel notification settings, so that I'm alerted like in Discord.
69. As a user, I want unread state to stay correct for servers I'm not looking at, so that badges are never stale.
70. As a user, I want read acknowledgements sent in a paced way, not one per incoming message, so that kyoko doesn't generate suspicious traffic.
71. As a user, I want to mark a channel, a server or everything as read, so that I can clear noise.
72. As a user, I want a tray icon with unread status and show/hide/quit, so that kyoko lives quietly in the background.
73. As a user, I want global hotkeys and single-instance behaviour, with `discord://`-style deep links opening in the running window, so that kyoko integrates with my desktop.

### Account safety

74. As a user, I want kyoko to present itself consistently as the Discord web client in both the gateway and REST, so that my session looks ordinary to Discord. That means a live build number, a consistent Chrome user agent and super-properties, a persisted cookie jar, my locale and timezone, and a Referer that follows the open channel.
75. As a user, I want kyoko to pace my write actions and respect Discord's rate limits learned from responses, so that heavy use doesn't look automated.
76. As a user, I want kyoko to stay within Discord's gateway send limit, counting heartbeats, so that the connection is never dropped for flooding.
77. As a user, I want kyoko to stop hitting a route that keeps returning "forbidden", so that repeated errors don't flag my account.
78. As a user, when Discord asks for a captcha in the app, I want kyoko to stop and tell me to complete the action in the official client, never retrying automatically, so that the request doesn't escalate into a temporary block.
79. As a user, I want kyoko to warn me and require confirmation before risky actions such as the following, so that I don't trip Discord's spam defences unknowingly:
    - messaging someone I've never talked to
    - replying in message-request or spam DMs
    - posting in empty channels
    - bypassing server screening, onboarding or verification
80. As an advanced user, I want a setting to turn those extra safeguards off at my own risk, so that I'm not blocked when I know what I'm doing.
81. As a user, I want kyoko to never log my token, authorization headers or raw gateway payloads, so that logs are safe to share.
82. As a user, I want kyoko to send no telemetry or analytics of its own, so that only Discord sees my traffic.

### Settings, appearance and performance

83. As a user, I want kyoko to look exactly like oxidcord, including its themes, so that I get the design I chose.
84. As a user, I want to switch themes and add them from a local file or a link, as Vencord does, so that I can change the look.
85. As a user, I want to import the colours of a Vencord or BetterDiscord theme, so that I can bring my palette across even though CSS layout tweaks can't apply.
86. As a user, I want settings for image quality (including original resolution), animation playback, reduced motion and the typing animation, so that I control looks against performance.
87. As a user, I want a setting to skip the "you're leaving Discord" warning on links, so that trusted links open directly.
88. As a user, I want server onboarding to appear without artificial delays, so that joining servers is quick.
89. As a user, I want search results to have the same full message context menu as the timeline, so that I can act on found messages directly.
90. As a user, I want kyoko to idle far below the official client's memory and CPU, with bounded caches that don't grow forever, so that it stays light all day.
91. As a user, I want kyoko's UI in my language, with oxidcord's nine translations carried over, so that it isn't English-only.

### Plugins (system)

92. As a user, I want to browse, enable, disable and configure plugins in settings, so that I can customise kyoko like Vencord.
93. As a user, I want each plugin to declare what it needs (network hosts, storage, UI surfaces, Discord data, actions) and ask me before it gets it, so that plugins can't do more than I allowed.
94. As a user, I want plugins to be unable to send messages, react or perform other Discord actions except as a direct result of my click or command, so that no plugin turns my account into a self-bot.
95. As a user, I want a misbehaving plugin to be stopped, disabled and reported without crashing kyoko, so that one bad plugin can't take the app down.
96. As a plugin author, I want to write plugins in Luau and see them hot-reload while I edit, so that iteration is fast.
97. As a plugin author, I want hooks for the following, so that I can build Vencord-style features natively:
    - before send
    - before edit
    - before typing
    - before notify
    - how user names display
    - relative-timestamp formatting
    - emoji autocomplete ranking
    - message rendering with access to kept deleted/edited versions
98. As a plugin author, I want UI slots for these, so that my plugin can add UI where Vencord plugins do:
    - user and member decorations
    - message accessories
    - composer and panel buttons
    - context-menu items
    - settings panes
99. As a plugin author, I want to subscribe to Discord events and query state through a stable, versioned, read-only API, so that my plugin keeps working across kyoko updates.
100. As a plugin author, I want per-plugin storage and settings, so that my plugin can remember configuration.

### Plugins (the first set)

101. As a user, I want MessageLogger, so that I can still see deleted messages and edit history inline, kept for a limited time.
102. As a user, I want ClearURLs, so that tracking parameters are stripped from links I send.
103. As a user, I want TextReplace, so that my own replacement rules rewrite outgoing text.
104. As a user, I want SilentTyping, so that I can hide that I'm typing, with a quick toggle in the composer.
105. As a user, I want ShowMeYourName, so that usernames show next to nicknames, or replace them.
106. As a user, I want PlatformIndicators, so that I can see whether people are on desktop, mobile or web.
107. As a user, I want ForceOwnerCrown, so that the owner crown shows even in large servers.
108. As a user, I want FavoriteEmojiFirst, so that my favourite emoji come first in autocomplete.
109. As a user, I want DontRoundMyTimestamps, so that relative timestamps round down.
110. As a user, I want OnePingPerDM, so that repeated DMs from the same person only ping me once until I read them.
111. As a user, I want ExpressionCloner, so that I can right-click an emoji or sticker and copy it into a server I manage.
112. As a user, I want ReadAllNotificationsButton, so that one click marks every server read.
113. As a user, I want GameActivityToggle, so that I can toggle sharing my game activity from the user panel once kyoko reports activities.
114. As a user, I want ShowHiddenChannels, so that I can see the channels I lack access to, with a lock and their name, topic, last activity and permission overwrites, without kyoko ever trying to load their messages.
115. As a user, I want ShowHiddenThings, so that moderator-only information the client already has is displayed regardless of my permissions, display-only.

### Developer experience

116. As a contributor, I want to run kyoko fully against mockcord with seeded data, so that I can develop and demo without a Discord account.
117. As a contributor, I want Luau UI changes to hot-reload in development, with scripts bundled as bytecode in release builds, so that iteration is fast and releases are tight.
118. As a contributor, I want one command that runs the session behaviour tests against mockcord, so that core regressions are caught before merging.
119. As a contributor, I want one command that captures screenshots of every screen from both oxidcord and kyoko against the same mockcord seed, so that I can check UI parity side by side.
120. As a contributor, I want CI on Windows to build kyoko, run the session behaviour tests and enforce formatting and lints, so that main stays green.

## Implementation Decisions

### Fork and repository

- kyoko is the GitHub fork `hoangvu12/kyoko` of `Master0fFate/oxidcord`, public, with `upstream` pointing at oxidcord.
- It was forked at oxidcord release 0.2.0-beta.1 (`b852253`). The core crates there are identical to the reviewed commit `497958b`; the newer commits only moved the GPUI UI to gpui-fast.
- Upstream changes are never merged wholesale. Useful core fixes are reviewed and hand-ported.
- Licence files, copyright line and NOTICE stay. The README gets a credit section.
- All oxidcord identity is replaced:
  - crate package names `oxidcord-*` become `kyoko-*`
  - binary name, app id, data directories, icon, user-agent-visible app names
  - update and catalog signing keys and endpoints

  oxidcord's name and icon are not reused.
- Contributor rules change in one respect: behaviour tests at the agreed seams are required. The other inherited rules stay:
  - tabs, fmt and clippy clean
  - no telemetry, never log secrets
  - lenient decoding
  - clean-room toward GPL clients
  - present only the web client's identity
  - never automate, bypass captchas/MFA/rate limits

### Crate disposition

- **Keep and fix:** `protocol`, `gateway`, `rest`, `state`, `store`, `login`, `media`, `platform`.
  - `markdown` stays as-is.
  - `platform` drops its single-instance module in favour of zgx's.
  - `login` keeps token and QR; web login is rebuilt on zgx's webview.
- **Replace:** `session` with a new, smaller session crate. oxidcord's is a reference only, because it hard-wires voice and the WebAssembly plugin host and crashes at startup if the plugin runtime fails.
- **Drop:** `ui` (GPUI), `voice`, `plugin-host` (WebAssembly), `plugin-sdk`, `updater`, plus the WebAssembly first-party plugins and the plugin pack files. Their behaviour is reimplemented where needed in the Luau plugin system.
- **Keep and extend:** `mockcord` and `xtask`, which remain the development and test tools.
- **New:**
  - the zgx desktop app (Rust host)
  - the Luau UI scripts
  - a Rust timeline view
  - the plugin runtime (Luau)
  - the core↔UI bridge

### Protocol and fingerprint

- **API version:** v9 for REST and gateway, matching the web client (oxidcord used v10).
- **Gateway transport:** JSON encoding with zlib-stream transport compression and one inflate context per connection. Inbound frames are capped at 64 MiB, because user READY payloads can exceed 16 MiB.
- **IDENTIFY:**
  - raw user token, no intents
  - web-client properties
  - `client_state` built from persisted cache versions
  - presence
- **Capabilities:** kyoko advertises only bits whose effects it implements. oxidcord's 32767 asks for three behaviours it doesn't handle: a refreshed token in READY, batched reaction events, and passive guild updates. Each extra bit is either fully handled or not advertised. Handling the refreshed token is required.
- **Client fingerprint** (one module, easy to update):
  - live build number scraped at startup, with a compiled fallback
  - reduced Chrome user agent in the `<major>.0.0.0` form
  - complete super-properties, consistent between gateway and REST
  - persisted installation id
  - persisted cookie jar shared by all REST requests
  - `X-Discord-Locale` and `X-Discord-Timezone`
  - `Referer` that follows the open channel
- **Gateway behaviour:**
  - heartbeat with a jittered first beat
  - ACK-timeout zombie detection leading to resume
  - resume vs re-identify state machine
  - INVALID_SESSION handling
  - a fatal close-code list that never reconnects
  - capped exponential backoff
  - a two-priority send queue inside a 120-per-60-seconds window that counts heartbeats
- **Lazy loading:**
  - op 37 bulk guild subscriptions, debounced and deduplicated
  - member-list ranges in 100-row buckets around the viewport (not just the first 100)
  - full GUILD_MEMBER_LIST_UPDATE handling (SYNC/INSERT/UPDATE/DELETE/INVALIDATE)
  - an op 8 member-request scheduler respecting Discord's rate-limited dispatches
- Parsing stays lenient everywhere. A debug log of raw dispatch types (never payloads containing secrets) supports diagnosing protocol drift.

### REST safety layer

- Rate-limit buckets are learned from response headers, never hard-coded. 429 handling honours `retry_after` and the global flag.
- **Pacing:** mutation requests are spaced, with message sends paced separately.
- **401:** stops all requests and returns the user to login.
- **403:** a per-route circuit breaker stops repeated forbidden calls.
- **Captcha:** a captcha response becomes a structured error the UI can explain. It is never retried automatically, and failed sends carry a reason.
- **Read acks:** debounced per channel, never one per incoming message.
- Endpoints user accounts can't use (such as bulk delete) are not called. Multi-delete uses paced single deletes.
- **Risky-action locks**, controlled by one safety setting that is on by default, which mark the composer and actions as locked with an explanation:
  - new DMs to strangers
  - message-request and spam DMs
  - empty channels
  - screening, onboarding and verification gates

### State and store

- State keeps the snapshot-plus-revision model: event, reducer, revision bump, UI refresh of changed areas.
- **Caches are bounded:**
  - recent messages per channel
  - a limited set of warm channels, beyond which bodies are dropped and metadata kept
  - profile and member caps
- Read-state and guild-settings versions are tracked and persisted, so `client_state` can shrink later READYs.
- Late-arriving private channels are inserted, not dropped.
- **Deleted and edited message retention** (for MessageLogger):
  - The store keeps deleted messages in place, flagged, and the previous versions of edited messages.
  - Both are capped per channel and by age, and wiped on sign-out.
- **Token:**
  - The token lives only in the OS keyring.
  - Plaintext storage requires an explicit opt-in setting with a visible warning; there is no silent fallback when the keyring errors.
  - The encrypted local cache stays, its key in the keyring.
- The local message table and the on-disk media cache are pruned by size and age.

### Session and the core↔UI bridge

- The new session owns a tokio runtime on background threads and is UI-agnostic. Its contract has three parts:
  - **Commands** in: login, open channel, load history, send/edit/delete, react, ack, subscribe ranges, and so on.
  - A **change notification** out.
  - **Snapshot reads** of state by area and revision.
- Voice and plugins are not dependencies of the session. Plugin hooks attach through an optional hook interface, and a plugin failure never stops the session.
- **Bridge from session to UI thread:**
  - Events cross on an unbounded inbox (queue plus waker) drained by a UI-thread task. zgx's latest-value channel is not used for events, because it drops intermediate values.
  - The UI thread reads the changed state areas and publishes small projections as zgx shared values: the visible guild/channel tree, the open channel's message window, the visible member ranges. The whole store is never published.
- UI actions from Luau go to Rust through zgx host actions and are forwarded to the session as commands.

### Media

- Fetching, decoding and caching happen in Rust.
- **Formats:** PNG, JPEG, GIF, WebP and animated WebP.
- **Cache:** byte-budgeted LRU.
- **Requests:** CDN and media-proxy URLs sized to device pixels.
- **Expired signed attachment URLs** are refreshed by re-fetching the message, never by retrying the stale URL.
- Decoded images reach the UI as host-supplied images, not through zgx's built-in remote image loader, which never evicts and spawns a thread per decode.
- **Animation:**
  - It pauses offscreen.
  - It honours the animation setting and reduced motion.
- An "original resolution" quality option replaces FixImagesQuality.

### UI on zgx

- **zgx is pinned** to `fc7d0f1e6695b2153bc0e18f0f1a3cef6f0426f6` (zgx is zgx renamed; the spike started on zgx `a9dc282`). The workspace copies zgx's required crate patches (winit, mlua, cosmic-text, taffy, wgpu-hal, four AccessKit adapters). Upgrades are deliberate.
- **The UI is a 1:1 port** of oxidcord's UI at release 0.2.0-beta.1:
  - layout, spacing, colours, icons, components, interactions and animations, screen by screen
  - the server rail and folders
  - channel and DM sidebars
  - top and title bars
  - the timeline and composer
  - the member list
  - the user panel
  - profile popout and page
  - the quick switcher
  - search
  - the lightbox
  - emoji and other pickers
  - threads and forums
  - friends
  - moderation and server settings
  - account and settings pages
  - login screens
  - toasts and transitions
- **Theme:** oxidcord's theme tokens (colours, UI font, code font) become zgx theme data. The theme catalog and its themes carry over.
- **Reused as logic:** emoji data and lookup, formatting helpers, and the Fluent translations (nine languages).
- **The timeline is a Rust view placed beside the Luau UI**, because zgx can't embed a Rust view inside a Luau tree. It is built on zgx's measured-row virtual list:
  - It follows the end of the conversation while the user is at the bottom.
  - It keeps the first visible row anchored when older history is prepended. zgx lacks this, so kyoko adds it.
  - It is styled to match oxidcord exactly.
- **Text rendering** will differ very slightly from GPUI's DirectWrite output. All other visuals must match.
- **Rebuilt rather than ported:** voice UI waits for voice; the updater screen waits for kyoko's own updater; the plugin settings screens are rebuilt for the Luau plugin system in oxidcord's visual style.
- Luau scripts hot-reload in development and are bundled as bytecode in release builds.

### Platform integration

- From `platform`, integrated on zgx's UI thread inside its event loop:
  - **tray**
  - **global hotkeys**
  - **OS notifications**, with an app user model id on Windows
  - **image clipboard**
  - **autostart**
- zgx provides single instance and deep links.
- **Windows is the only v1 platform.** Development, CI, packaging and validation target Windows (x86_64, MSVC). Linux and macOS code paths inherited from oxidcord may be removed or left uncompiled. No effort goes into keeping them working. In particular, the zgx web login is WebView2-only, and zgx has no Linux webview.
- Packaging is Windows only: oxidcord's per-user installer and portable zip, with kyoko's identity. Oxidcord's macOS and Linux packaging and release jobs are dropped.

### Login

- **Token:** validated before saving.
- **QR:** the remote-auth gateway with its RSA handshake. Oxidcord's implementation is kept.
- **Web login** in a zgx WebView2 window:
  - an ephemeral profile
  - the same reduced Chrome user agent as the client, to avoid a device mismatch
  - the token read from Discord's page storage by a document-start script restricted to discord.com
  - browsing data cleared afterwards
  - It's acknowledged as brittle if Discord changes how the page stores the token.
- **Email/password:**
  - login, then the 2FA step (TOTP, SMS, backup code)
  - any captcha response hands the user to web login

### Plugin system

- **One contract, owned by the core and versioned.** Each plugin is Luau and has two halves:
  - **Logic half:** data hooks run in a sandboxed Luau VM on a worker thread, with call deadlines, so per-message hooks never block the UI thread.
  - **UI half:** a zgx plugin that contributes to named UI slots. It reaches Discord data only through capabilities the app exposes by name.
- **Plugin manifest** declares:
  - permissions (network hosts, storage, UI slots, Discord read scopes, action scopes)
  - settings schema
  - version
  - the user approves the permissions on enable
- **Data access:**
  - Plugin access to shared Discord data is read-only; plugin writes to app state are ignored.
  - There is no API to fake client features or bypass permission checks.
- **Discord actions:**
  - only inside a user gesture (a UI click or command)
  - rate-limited per plugin
- A plugin that errors repeatedly or exceeds its deadlines is disabled with backoff and a visible notice. It never crashes the app.
- **Hook API v1** (derived from the chosen plugin list):
  - **Text and notifications:** before-send/before-edit text rewrite; before-typing veto; before-notify filter.
  - **Display:** user display-name formatting; relative-timestamp formatting; emoji autocomplete ranking.
  - **Data:** message render access to kept deleted/edited versions; event subscription; read-only state queries.
  - **UI slots:** user/member decoration, message accessory, composer and panel buttons, context-menu items, settings pane.
  - **Actions:** emoji/sticker upload, bulk mark-read, activity toggle.
  - **Storage:** per-plugin storage.
- **First plugins:**
  - **Plugins on the hook API:** MessageLogger, ClearURLs (rules bundled, no network fetch), TextReplace, SilentTyping, ShowMeYourName, PlatformIndicators, ForceOwnerCrown, FavoriteEmojiFirst, DontRoundMyTimestamps, OnePingPerDM, ExpressionCloner, ReadAllNotificationsButton, GameActivityToggle.
    - GameActivityToggle depends on kyoko reporting activities. Until then it ships disabled.
  - **First-party, display-only:** ShowHiddenChannels and ShowHiddenThings. They read data the client already has and never fetch from hidden channels.
- **Built-in behaviour** instead of plugins:
  - ValidReply and ValidUser (background backfill of missing referenced messages and users, cached and paced)
  - FixImagesQuality (original-resolution option)
  - NoTypingAnimation (setting plus reduced motion)
  - AlwaysTrust (link-warning setting)
  - NoOnboardingDelay (no artificial delay in onboarding)
  - FullSearchContext (search results share the timeline's context menu)
- **Themes:**
  - zgx theme data with kyoko's token set
  - an importer that takes only colour variables from Vencord/BetterDiscord CSS
  - Themes come from the bundled set, from local files in the themes folder, or from links the user adds, as Vencord's local and online themes do. A theme is a kyoko `theme.json` or a Vencord/BetterDiscord `.css` file whose colour variables are imported. No signing key and no catalog server (decided 2026-10-26: themes carry no code, so a signature adds nothing over HTTPS and validation).

### Milestones

1. **Spike** (Windows, about one week): the end-to-end proof in story 10.
2. **Core:**
   - fork renaming and cleanup
   - new session and bridge
   - gateway, REST, fingerprint and safety fixes
   - state and store fixes
   - media
   - session behaviour tests and mockcord wire recording
3. **Logins:** token, QR, web, email/password.
4. **UI parity:** screen-by-screen 1:1 port, each screen verified by screenshot comparison.
5. **Plugins:** the runtime, hook API v1, the first plugin set, built-ins, themes.

## Testing Decisions

- **Good tests check external behaviour only:** what the user or Discord would observe. They never check internal structure. Tests are deterministic: seeded mockcord data, and controlled time for pacing and backoff.

**Seam 1: the session boundary against mockcord (primary).**

- Tests start an in-process mockcord with a seed, create a session pointed at it, send commands, and assert two things:
  - The state snapshots and events the session publishes.
  - What mockcord observed on the wire:
    - IDENTIFY contents and capabilities
    - the REST header set and fingerprint consistency
    - request spacing and pacing
    - the gateway send window
    - resume vs re-identify after injected disconnects
    - fatal close codes
    - stop-on-401
    - the 403 breaker
    - the captcha stop with no retry
    - ack debouncing
    - member-list range subscriptions
    - backfill pacing
- Mockcord gains two things:
  - A traffic-recording capability that tests can query.
  - Scenario controls to inject responses and closes: 401, 403, 429 with retry-after, captcha errors, close codes, resume failures, late private channels, capability-dependent dispatches.
- Login flows (token, QR, email/password with 2FA and captcha hand-off) are tested through the same seam against mockcord's auth endpoints. The web-login page itself is checked manually.
- Plugin hooks are tested through the same seam with test plugins: hooks fire on the right commands and events, permissions are enforced, gesture-only actions are refused outside gestures, deadline overruns disable the plugin.
- **Prior art:** oxidcord's headless runner already drives a session with commands against an in-process mockcord, using seeds and a chaos mode. The behaviour tests follow that pattern as proper test cases with assertions.

**Seam 2: UI parity by screenshot comparison.**

- oxidcord (pinned at 0.2.0-beta.1) and kyoko run against the same mockcord seed, and every screen and state in the parity list is captured from both.
- A screen ticket is done when its kyoko screenshots match oxidcord's side by side, allowing only the documented text-rendering differences.
- A capture command produces both sets for review.

**Real Discord.**

- Checked by hand only, with the maintainer's throwaway account and a token supplied through an environment variable.
- Never in CI, never committed, never logged.

**CI** (Windows only):

- build
- fmt
- clippy with warnings denied
- the session behaviour tests

The parity captures run on demand.

## Out of Scope

- **Voice and media:** voice calls, video, screen share and stream watching, along with their UI and the VolumeBooster and DisableCallIdle plugins, until a later voice effort.
- **Updating and oxidcord's plugin stack:** kyoko's own auto-updater (oxidcord's updater is dropped), oxidcord's WebAssembly plugin host, SDK and plugins, and a TypeScript or JavaScript plugin authoring path.
- **Real Vencord:** running Vencord, Vencord plugin code or Vencord CSS themes. Only colour variables are imported from themes.
- **Plugins we never ship:** any that fake client features (FakeNitro, SpotifyCrack, Experiments-style unlocks), automate actions or presence in the background, or bypass permissions beyond display-only.
- **Web-only plugins:** CrashHandler, F8Break, NoDevtoolsWarning, ReactErrorDecoder and YoutubeAdblock. These are web-client specific; kyoko relies on native panic handling and zgx's script error reporting instead.
- **Things kyoko won't do:** solving or bypassing captchas, and any self-bot or automation feature.
- **Platforms and distribution:** Linux and macOS builds, packaging and CI (deferred); mobile clients; app-store distribution.

## Further Notes

- **Account risk.** Using a third-party client with a user account can violate Discord's Terms of Service. Discord's support policy explicitly forbids automating user accounts. The safety layer reduces risk but can't remove it. The login-screen notice and the README disclaimer stay.
- **Clean-room rules.** concord (GPL-3.0) and Vencord (GPL-3.0-or-later) are behavioural references only: protocol facts and feature ideas, never code or close paraphrase. oxidcord's own code is MIT/Apache and may be reused with attribution.
- **zgx maturity.** zgx is at 0.1 and changes quickly, and Windows is its least automatically tested platform. The spike exists to surface problems early. Upgrades are planned events.
- **Capability bits.** The exact set to advertise, and whether the current web client still sends what concord uses (253), should be confirmed against real traffic during the spike.
- **Fingerprint drift.** Chrome version and build-number discovery can change without notice. The fingerprint module is the one place to update.
- **Web login fragility.** Web login depends on how Discord's page stores the token. If that changes, token and QR login remain.
- **Other platforms.** Linux and macOS are deferred, not abandoned. The core crates stay platform-neutral where that costs nothing, so a later port mostly concerns the zgx host, login webview, platform integrations and packaging.
- **Background research.** Citations for every protocol and framework fact are in the research notes kept in the repository: the zgx and Discord core research, the oxidcord review and the Vencord triage.
