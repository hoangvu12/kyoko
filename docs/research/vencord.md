# Vencord on a native zgui Discord client

Date: 2026-10-25. This builds on `core.md` in this folder (core, ToS, account risk), which this note doesn't repeat.

Sources, all cloned read-only into fresh temp dirs. Nothing was built or run.
- `Vendicated/Vencord` @ `443688b` (2026-10-24, v1.15.13, GPL-3.0-or-later).
- `Vencord/Vesktop` @ `3cb2a7b` (2026-10-24, GPL-3.0-or-later).
- `oxidcord` @ `497958b` (MIT OR Apache-2.0).
- `zeronsh/zgui` @ `a9dc282` (MIT).
- Upstream web docs, cited as URLs.

Every claim is tagged **[read]** (seen in the source or document cited) or **[inferred]** (my reasoning, not verified). Paths are relative to each repo root.

---

## Recommendation

**Can we run Vencord? No.** Vencord isn't a plugin API. It's a set of regex patches on the source text of Discord's minified webpack modules, plus React components and Flux stores taken from inside that bundle (§1.2–1.3).
- A native client has no Discord bundle, no webpack, no React and no DOM, so Vencord and its plugins have nothing to attach to (§2).
- The only way to get real compatibility is to run Discord's own web app in a web view and inject Vencord into it. That is what Vesktop does (§2.1).
  - zgui could technically do the same. Its WebView2 pages support document-start user scripts (§2.1).
  - But that is an Electron client with extra steps: the Discord UI would be the web one, not ours. It adds back the browser processes we are trying to avoid, and it is exactly the "unauthorized software designed to modify the services" the ToS names.
- Shimming webpack, Flux and React is not realistic: it means re-creating Discord's client (§2.3).

**Can we have a similar system? Yes, and the useful part of it.** Of Vencord's 168 stock plugins (§3):

| Class | Count | What it means for us |
| --- | --- | --- |
| (A) logic/data only | 25 | Portable as plugins once hooks exist (pre-send, filter, events, commands, storage) |
| (B) UI-surface additions | 53 | Portable given slots: context menus, message accessories and decorations, composer buttons, badges, panels |
| (C) deep UI patches | 58 | Not portable. Native built-ins, and many are just a setting in our own UI |
| (D) irrelevant to a native client | 32 | Electron, browser, devtools or Discord web quirks |

- **Code doesn't port, behaviour does.** The A and B plugins (78) are re-implementations of the behaviour, not ports of code: almost every one calls Discord's internal modules.
- **Licence.** Vencord is GPL-3.0-or-later (§1.7), so even copying logic would make that plugin GPL. Write ours from the behaviour, in clean room.
- **Account risk.** 33 plugins carry a risk flag. Three of them fake client features (FakeNitro, SpotifyCrack, Experiments) and should never be offered.

**Recommended design (§4):** one API contract, one language, two VMs.

1. **A versioned, permissioned hook contract owned by the Rust core.** Use oxidcord's WIT and its policies as the model: per-hook deadlines, per-call permissions, rate limits, crash backoff, a render cache, signed catalogs and a conformance runner. Extend it with the hooks Vencord shows people use (§4.2).
2. **Luau as the plugin language,** since zgui already links Luau through `mlua`. Each plugin gets two halves:
   - a **logic half** in a sandboxed `mlua` Luau VM on a core worker thread, for data hooks off the UI thread;
   - a **UI half** that is an ordinary zgui plugin contributing to `ui.slot`s. It reaches Discord data only through `expose_shared` / `expose_action` names gated by `Capability::Named("discord.…")`.
3. **WASM components (oxidcord's `plugin-host`) as an optional later backend** behind the same contract, for Rust and JS authors (JS via ComponentizeJS). No TS/JS authoring path in v1 (§4.4).
4. **Themes are token files, not CSS** (§4.5):
   - zgui theme JSON plus a `discord.*` app-token namespace;
   - an importer that keeps only the colour variables of a Vencord/BetterDiscord theme.

**First plugins to ship** (§5): Message Logger (core retention plus UI), Clear URLs, Silent Typing, Text Replace, No Reply Mention + Silent Message, Show Me Your Name / IRC Colors, Platform & Typing indicators, Translate, and a "Copy & Inspect" context-menu bundle (View Raw, Copy User URL, Reverse Image Search, Permissions Viewer).

---

## 1. How Vencord works

### 1.1 Injection

**Discord desktop (Electron patcher).**
- **Installation.** The installer (a separate project, downloaded from `github.com/Vencord/Installer/releases`, `scripts/runInstaller.mjs:28`) replaces Discord's `app.asar` with Vencord's injector and keeps the original as `_app.asar`. `src/main/patcher.ts:28-41` finds the original asar, rewrites `require.main.filename` and calls `app.setAppPath(asarPath)` **[read]**.
- **Hooking windows.** It subclasses `BrowserWindow` and swaps every Discord window's preload for Vencord's, setting `sandbox = false`. It stores Discord's own preload in `process.env.DISCORD_PRELOAD` (`patcher.ts:70-81,105`), then replaces `electron`'s exports with the subclass (`patcher.ts:121-127`) **[read]**.
- **Devtools.** It force-enables devtools via `appSettings` (`patcher.ts:129-132`), then loads the original Discord app (`patcher.ts:139-140`) **[read]**.
- **Preload.**
  - The preload exposes `VencordNative` through `contextBridge` (`src/preload.ts:25`).
  - It runs the renderer bundle with `webFrame.executeJavaScript(...)`.
  - Then it `require`s Discord's original preload (`preload.ts:31-35`) **[read]**.
- **Surviving Discord updates.** It wraps `EventEmitter.prototype.emit` to catch Discord's `host-updated` event. It then copies its asar over the new version's `app.asar` (`src/main/persistAfterDiscordUpdates.ts:62,72`) **[read]**.

**Browser extension.**
- **Content scripts.** It is MV3 with two content scripts at `document_start` on `*://*.discord.com/*`, one in the `ISOLATED` world and `dist/Vencord.js` in the `MAIN` world (`browser/manifest.json:20-35`) **[read]**.
- **CSP.** A `declarativeNetRequest` rule strips `content-security-policy` and `content-security-policy-report-only` from discord.com frames (`browser/modifyResponseHeaders.json:1-20`) **[read]**.
- **Userscript.** A userscript build also exists (`browser/userscript.meta.js`) **[read]**.

**Vesktop:**
- **What it is.** Its own Electron app (`electron` 45.0.0, `package.json:41`, GPL-3.0-or-later `package.json:8`). The main window is `sandbox: true, contextIsolation: true` with its own preload (`src/main/mainWindow.ts:353-358`), and it loads `https://${subdomain}discord.com/app` (`mainWindow.ts:457`) **[read]**.
- **Injection.** Its preload runs the Vencord preload via `Function("require","process", …)`, then injects the Vencord and Vesktop renderer scripts with `webFrame.executeJavaScript` (`src/preload/index.ts:16-19`) **[read]**.
- **Extras.** `arrpc` for local Rich Presence and `@vencord/venmic` for Linux screen-share audio (`package.json:28,32`) **[read]**.
- **In short,** Vesktop is "Discord's web app, in our own Electron shell, with Vencord preinstalled" **[read; summary]**.

**In all three modes, Vencord runs as JavaScript inside the page that runs Discord's web bundle** **[read]**.

### 1.2 Webpack module patching

**Intercepting webpack.** `src/webpack/patchWebpack.ts` defines a setter for `m` on `Function.prototype` (`:97-100`). When Discord's webpack runtime assigns `wreq.m` (its module-factory table), Vencord:
- checks the call stack and bundle path to pick Discord's main instance, skipping `sentry`, `libdiscore` and `fast-connect` (`:103-117,132`);
- wraps every factory in a `Proxy` (`:183-218,251-283`);
- patches either eagerly or on the factory's first call (`:87-95,287`) **[read]**.

**Applying patches.**
- `patchFactory` (`:504-646`) takes `String(originalFactory)`, the module's *source text*. For each registered patch whose `find` (a string or RegExp) matches it, it runs every `replacement` as `code.replace(match, replace)` (`:529-531,545-550`).
- It then **re-`eval`s** the result into a new factory, with a `sourceURL` naming the patching plugins (`:593-594`).
- A no-op replacement logs "had no effect".
- `group: true` patches roll back as a unit (`:571-583,623-631`).
- `fromBuild`/`toBuild` gate patches to Discord build numbers (`:518-526`) **[read]**.

**Patch shape** (`src/utils/types.ts:72-111`):
- `find`, `replacement: {match, replace, noWarn?, predicate?, fromBuild?, toBuild?}`, plus `all`, `group` and `predicate` **[read]**.
- `match` patterns use Vencord's `\i` identifier shorthand against minified code. An example is the MessageEvents API hooking `.handleSendMessage,onResize:` (`src/plugins/_api/messageEvents.ts:38-46`) **[read]**.

**Finds** (`src/webpack/webpack.ts`). These search the live module cache:
- **Filters** (`:61-113`): `filters.byProps(...)`, `byCode(...)` (on `Function.prototype.toString`), `byStoreName` (`constructor.displayName`), `componentByCode` (unwraps memo and forwardRef) and `byClassNames` **[read]**.
- **Entry points:**
  - `find`/`findAll`/`findBulk` (`:228-289`);
  - `findByProps` (`:438`) and `findByPropsLazy` (`:448`);
  - `findByCode`/`findByCodeLazy` (`:457,467`);
  - `findStore` (`:500`, which first fills a map from `Flux.Store.getAll()` and from `libdiscoreWasm` exports ending in `Store`, `:473-497`);
  - `findComponentByCode`/`findComponentLazy` (`:531-570`);
  - `findCssClasses` (`:596`);
  - `mapMangledModule` (`:643`);
  - `waitFor` (`:773`) **[read]**.
- **Lazy variants** return a `proxyLazy` that resolves on first property access, so plugins can declare finds at module scope before webpack has loaded (`:400-420`) **[read]**.

**Scale at this commit:** 358 `find:` patches across 147 files in `src/plugins` (including the `_api`/`_core` plugins). In stock plugins, 129 of 168 use at least one patch (327 patches) **[read; counted with grep]**.

### 1.3 Discord's Flux stores, dispatcher and React components

- **Shared handles.** `src/webpack/common/stores.ts:26-92` exports lazily found handles to `Flux` and about 60 Discord stores:
  - `MessageStore`, `ChannelStore`, `UserStore`, `GuildMemberStore`, `PresenceStore`, `TypingStore`, `RelationshipStore`, `VoiceStateStore`, `UserSettingsProtoStore`, `SpotifyStore`, `ExperimentStore` …
  - `src/webpack/common/components.ts`, `menu.ts` and `modals.ts` do the same for Discord's React components (for example `Menu.MenuItem`) **[read]**.
- **Plugins render Discord's own components.** `CopyUserURLs` pushes a `<Menu.MenuItem>` into Discord's user context menu (`src/plugins/copyUserURLs/index.tsx:33-56`) **[read]**.
- **Flux handlers.**
  - A plugin's `flux: { EVENT(e) {} }` map (`types.ts:183-185`) is subscribed with `FluxDispatcher.subscribe` on start and removed on stop (`src/api/PluginManager.ts:161-192`) **[read]**.
  - `FluxEvents` lists 2,260 event names (`packages/discord-types/src/fluxEvents.d.ts:22`) **[read; counted]**.
  - Stock plugins actually subscribe to only a couple of dozen. The most common are `CONNECTION_OPEN`, `CHANNEL_SELECT`, `LOGOUT` and `MESSAGE_CREATE/DELETE` **[read; grep]**.
- **Internals as API.** Plugins also mutate internal state by dispatching (`FluxDispatcher.dispatch`), and call Discord's REST wrapper `RestAPI.get/post/patch` with the user's session (for example `whoReacted/index.tsx:46-74`, `messageLinkEmbeds/index.tsx:133-160`) **[read]**.
- **Store-level patching.** `MessageLogger` patches `MessageStore`'s `MESSAGE_DELETE` handler to mark messages `deleted` instead of removing them (`src/plugins/messageLogger/index.tsx:479-492`) **[read]**.

### 1.4 Plugin API surface

**`definePlugin`** (`src/utils/types.ts:32`) is an identity function over `PluginDef` (`:126-221`). Its fields:
- **Metadata:** `name`, `description`, `authors`, `tags`.
- **Lifecycle:** `start`/`stop`, and `startAt` (`Init`, `DOMContentLoaded` or `WebpackReady`, `:223-230`).
- **Code:** `patches`, `commands`, `dependencies`, `required`/`enabledByDefault`, and `requiresRestart` (defaults to true when the plugin has patches, `PluginManager.ts:116`).
- **Settings and hooks:** `settings`, `flux`, `contextMenus`, `toolboxActions` and `managedStyle`.
- **Shorthands:** `userProfileBadge`, `messagePopoverButton`, `chatBarButton`, `onMessageClick`, `onBeforeMessageSend`, `onBeforeMessageEdit`, `renderMessageAccessory`, `renderMessageDecoration` and `renderMemberListDecorator`.
- **Wiring.** `PluginManager.startPlugin` wires each shorthand to the matching API (`PluginManager.ts:205-260`) **[read]**.

**Settings:**
- `OptionType` is `STRING, NUMBER, BIGINT, BOOLEAN, SELECT, SLIDER, COMPONENT, CUSTOM` (`types.ts:243-252`).
- Options can have `isValid`, `disabled`/`hidden` predicates, `restartNeeded` and a `WEB|DESKTOP|BOTH` target (`:270-301`).
- Settings render into Vencord's settings tab **[read]**.

**APIs (`src/api/`).** Each API is a small registry plus an "API plugin" in `src/plugins/_api/` that patches Discord's component to call it. For example, MessageAccessories splices `Vencord.Api.MessageAccessories._modifyAccessories(...)` into the accessories `children` array (`src/plugins/_api/messageAccessories.ts:28-31`) **[read]**.

| API | Registration | What a plugin gets | Source |
| --- | --- | --- | --- |
| Commands | `registerCommand(cmd, plugin)` / `commands: []` | Discord slash-command UI with options, executed locally | `api/Commands/index.ts:144` |
| MessageEvents | `addMessagePreSendListener`, `addMessagePreEditListener`, `addMessageClickListener` | mutate `{content, …}` or `{cancel:true}` before send/edit; click handler | `api/MessageEvents.ts:57-58,94-135` |
| MessageAccessories | `addMessageAccessory(id, factory, position)` | React node under a message | `api/MessageAccessories.tsx:31` |
| MessageDecorations | `addMessageDecoration(id, factory)` | React node beside the author name | `api/MessageDecorations.tsx:53` |
| MessagePopover | `addMessagePopoverButton(id, factory)` | hover-bar button | `api/MessagePopover.tsx:60` |
| ContextMenu | `addContextMenuPatch(navId, cb)`, `addGlobalContextMenuPatch` | mutate the `children` of any Discord menu by `navId` | `api/ContextMenu.ts:45-85` |
| Badges | `addProfileBadge(badge)` | profile badge with position, link and tooltip | `api/Badges.ts:66` |
| ServerList | `addServerListElement(position, Component)` | component above/below the guild rail | `api/ServerList.tsx:34` |
| MemberListDecorators | `addMemberListDecorator(id, render, onlyIn)` | node next to member-list / DM-list entries | `api/MemberListDecorators.tsx:37` |
| ChatButtons | `addChatBarButton(id, render, icon)` | composer button | `api/ChatButtons.tsx:125` |
| Notices | `showNotice(message, buttonText, onOk)` | Discord's top notice bar | `api/Notices.tsx:42` |
| Notifications | `showNotification(data)` | in-app toast + optional desktop notification, persisted log | `api/Notifications/Notifications.tsx:95`, `notificationLog.tsx:47` |
| DataStore | `get/set/update/del/keys/createStore` | IndexedDB key-value (a vendored idb-keyval, Apache-2.0) | `api/DataStore/index.ts:1-17,32-256` |
| Styles | `requireStyle/enableStyle/disableStyle`, `managedStyle` | `<style>` nodes under a `<vencord-root>` element | `api/Styles.ts:36-146` |

All rows are **[read]**.

### 1.5 Themes and QuickCSS

- **QuickCSS** is a `<style id="vencord-custom-css">` whose `textContent` follows `VencordNative.quickCss` (`src/api/Themes.ts:32-46`) **[read]**.
- **Themes.**
  - Online theme links and local theme files become `@import url(...)` lines in a `vencord-themes` style element (`Themes.ts:51-95`).
  - Local files are served on desktop through a `vencord:///themes/` protocol (`src/main/index.ts:33-49`).
  - A BetterDiscord meta parser reads theme headers (`src/main/themes/index.ts:4-6`, Apache-2.0 from BetterDiscord) **[read]**.
- **What themes target.**
  - Themes are plain CSS against Discord's DOM: its hashed class names (Vencord itself needs `makeClassNameRegex`/`findCssClasses` to resolve them, `webpack.ts:57,596`) and its CSS custom properties **[read]**.
  - `themeAttributes` adds `data-*` attributes to Discord's DOM so themes have stable selectors **[read; plugin description]**.
- **Vencord's own rule.** "No plugins that simply hide or redesign ui elements. This can be done with CSS" (`CONTRIBUTING.md:53`). In other words, much of Discord customisation lives in CSS, not plugins **[read]**.

### 1.6 userplugins

- **Build-time only.** The build globs `plugins/_api`, `plugins/_core`, `plugins` and `userplugins` and compiles them into the bundle (`scripts/build/common.mjs:148-155`). `src/userplugins` is git-ignored (`.gitignore:22`) **[read]**.
- **No runtime loading and no sandbox.** A userplugin has the same full page access as a stock plugin **[read: build glob; inferred: no isolation, since everything shares one JS realm]**.
- **Docs:** "To add custom plugins, you need to build Vencord from source", the instructions are "provided for advanced users", and "We do not provide support with custom plugins / installs" (https://docs.vencord.dev/installing/custom-plugins/) **[read]**.

### 1.7 Licence and what porting implies

- **The licence.**
  - `package.json:14`: `"license": "GPL-3.0-or-later"`. `LICENSE` is the GNU GPL v3 text.
  - Every source file carries: "This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version." (`src/main/patcher.ts:5-8`) **[read]**.
  - Exceptions are vendored files: `api/DataStore` (idb-keyval) and `main/themes` (BetterDiscord meta parser) are Apache-2.0 **[read]**. Vesktop is GPL-3.0-or-later as well **[read]**.
- **What the GPL FAQ says** (https://www.gnu.org/licenses/gpl-faq.html) **[read]**:
  - #TranslateCode: "translation of a work is considered a kind of modification" and "The translation is covered by the copyright on the original program". So rewriting a Vencord plugin's TS logic in Luau or Rust is still a derivative.
  - #GPLPlugins: "If the main program dynamically links plug-ins, and they make function calls to each other and share data structures, we believe they form a single combined program". With fork/exec and simple communication they are separate programs.
  - #NFUseGPLPlugins: if they form a single combined program, "the main program must be released under the GPL or a GPL-compatible free software license, and the terms of the GPL must be followed when the main program is distributed for use with these plug-ins."
- **Implication for us:**
  - A ported (translated) Vencord plugin is GPL-3.0+ **[inferred from the FAQ]**.
  - Shipping it inside our app, or as a first-party plugin talking to our host over a rich hook API, likely makes the distributed combination subject to GPL terms. Our MIT/Apache core stays MIT/Apache file by file (MIT is GPL-compatible), but the bundle is distributed under GPL **[inferred]**.
  - The GPL protects code, not ideas: re-implementing a feature from its description (for example "strip tracking params before send") is not a derivative **[inferred]**.
- **Rule:** first-party plugins are clean-room re-implementations from plugin descriptions and observed behaviour. Never copy Vencord regexes, rule lists or logic. Third-party GPL plugins can exist, but we don't bundle them **[inferred]**.

---

## 2. Can Vencord itself run in a native client?

**No.** Every part of Vencord assumes four things that a Rust core with a zgui/Luau UI doesn't have:

1. **Discord's webpack bundle in the same JS realm.** Patches are regexes over the source text of Discord's minified modules (§1.2). Without Discord's JS there is no source to patch.
2. **Discord's React tree.** API plugins splice calls into Discord's components (§1.4), and plugins render Discord's own components, such as `Menu.MenuItem`.
3. **Discord's Flux stores and dispatcher.** Plugins read and write them (§1.3).
4. **The DOM and CSS.** These are needed for themes, QuickCSS and `managedStyle` (§1.5).

Only 39 of 168 stock plugins have no patch, and even those use Discord components, stores or `RestAPI` (for example `copyUserURLs` uses `Menu.MenuItem`, and `clearURLs` hooks Vencord's MessageEvents API, which itself is a patch) **[read]**.

Real compatibility can come only from the routes below.

### 2.1 Embed Discord's web app in a web view (what Vesktop is)

- **How it would work in zgui.**
  - zgui has `ui.webview`/`ui.web` behind `Capability::Web` (`docs/scripting.md:136,328`).
  - On Windows it uses WebView2, with "user scripts" and app schemes (`docs/windows.md:224-250`).
  - Scripts are "injected at document start, in order, before the page's own" (`crates/zgui/src/web_view.rs:240`), via `AddScriptToExecuteOnDocumentCreated` (`crates/zgui-desktop/src/native_host/webview2.rs:742-749`) **[read]**.
  - Loading `https://discord.com/app` and injecting Vencord's web build (`dist/Vencord.js`, the same file the extension runs in `MAIN` world at `document_start`) is the Vesktop recipe **[inferred]**.
  - CSP stripping (`modifyResponseHeaders.json`) would need a WebView2 response filter, or acceptance that the userscript build works under CSP **[inferred; untested]**.
- **What you get:** 100% Vencord, but Discord's UI, not ours. zgui becomes a frame around a browser **[inferred]**.
- **Memory.**
  - WebView2 runs the browser, renderer, GPU and utility processes of Edge's runtime ("the runtime is the system's", `windows.md:245`) **[read]**.
  - So the footprint is close to Vesktop or Discord desktop, plus zgui's own baseline (about 103–116 MB idle, `core.md` A §8) **[read for zgui; inferred for the total; no measurement of Discord-in-WebView2 was found or made]**.
- **Complexity:** low to build, since Vesktop proves the pattern. But it makes the native core pointless for that surface, and the two clients (native and web) would both hold sessions **[inferred]**.
- **ToS:**
  - Discord's ToS prohibits "using any unauthorized software designed to modify the services" (`core.md` §7.3, https://discord.com/terms) **[read]**.
  - Vencord's own README says "Client modifications are against Discord's Terms of Service" (`README.md:55-57`) **[read]**.
  - Risk-wise, this route matches running Vencord today. It is lower than a native client on the fingerprint axis, since it is the real web client. Detection of client mods exists: `noTrack` patches Discord's `hasClientMods` check (`find: ".BetterDiscord||null!="`, `src/plugins/_core/noTrack.ts:65-71`) **[read]**.

### 2.2 Hybrid: only some surfaces web

- **The idea.** Native shell, with selected surfaces (Activities, maybe settings or a "web fallback" tab) in a `ui.webview` running Discord's web app, with or without Vencord **[inferred]**.
- **What Vencord would cover.** Only what lives inside the web surface. Vencord can't reach native surfaces, and the web surface would load Discord's whole bundle anyway (Discord doesn't ship per-surface bundles) **[inferred]**.
- **Cost.**
  - Memory: the full web-view cost from §2.1 whenever the surface is open.
  - Complexity: two sources of truth for state, unless the web surface is isolated. A second gateway session from the web view would look like a second client to Discord **[inferred]**.
- **ToS:** same as §2.1 for the web part, on top of the native client's own exposure (`core.md` §7) **[inferred]**.
- **Verdict:** only reasonable for features we will never build natively (Activities), and then without Vencord **[inferred]**.

### 2.3 Shimming webpack, Flux and React in a native client

**Why it is infeasible:**
- **Patches need Discord's code, not just an API.** To run Vencord's patches you must provide modules whose *source text* matches Discord's current minified code: 327 regex patches in stock plugins keyed on strings like `.handleSendMessage,onResize:` (§1.2). They are maintained against Discord's live builds ("broken plugins are usually fixed within 12 hours", `README.md`) **[read]**. So you would have to run Discord's real code, which is §2.1.
- **Shims would have to mirror Discord's internals.** You would need about 60 Flux stores with their exact method names (`stores.ts:32-92`) and 2,260 dispatcher event types (§1.3). You would also need Discord's React component library, rendered to zgui, plus `RestAPI` and `Constants.Endpoints` **[read: counts; inferred: infeasibility]**.
- **The bottom line.** That amounts to writing Discord's web client again in JS on a React-to-zgui reconciler. It would need a JS engine and the bundle, and would break with every Discord build **[inferred]**.

| Route | Vencord compatibility | Memory (relative) | Complexity | ToS exposure |
| --- | --- | --- | --- | --- |
| Web view (Vesktop-style) | Full | Browser-class, the native win is gone **[inferred]** | Low | Same as Vencord: "unauthorized software designed to modify the services" |
| Hybrid surfaces | Partial, web surfaces only | Browser-class while the surface is open **[inferred]** | High (dual state/session) | Native client + Vencord |
| Shim webpack/Flux/React | None in practice | n/a | Prohibitive | n/a |
| **Native "Vencord-like" API (recommended)** | Behaviour of ~78 A/B plugins re-implemented | Luau VM ≈ +0.9 MiB per script in zgui's measurement (`docs/scripting.md:495`) **[read]**; caps of 32 MiB per plugin VM (`:24`) | Medium | Native-client exposure only (`core.md` §7). Our API decides what plugins may do |

---

## 3. Plugin portability triage (all 168 stock plugins)

`src/plugins/` at `443688b` has 168 plugin directories, excluding `_api` and `_core` **[read]**.

**Method.**
- Each plugin is classified from its `definePlugin` description, the APIs it registers and its patch count. The scan grepped for `find:` patches, `flux`, `commands`, `contextMenus`, render shorthands, `RestAPI`, `fetch` and `sendMessage`.
- Plugins needing voice are classified by the surface they touch, since voice is outside the MVP (`core.md`).
- Classification is **[inferred]**. Counts are **[read; counted]**.

| Class | Plugins | With ≥1 patch | Patches |
| --- | --- | --- | --- |
| A logic/data | 25 | 15 | 40 |
| B UI-surface additions | 53 | 36 | 76 |
| C deep UI patches → native built-in | 58 | 53 | 144 |
| D irrelevant | 32 | 25 | 67 |
| **Total** | **168** | **129** | **327** |

### 3.1 What oxidcord already covers

oxidcord's first-party plugins (`plugins/README.md:5-17`, manifests in `plugins/*/plugin.toml`) map to Vencord as follows **[read]**:

| oxidcord plugin | Vencord equivalent | Notes |
| --- | --- | --- |
| `link-cleaner` (`message-send`) | ClearURLs | Same hook (pre-send rewrite) |
| `text-shortcuts` (`message-send`, `storage`, `ui-slots`) | TextReplace | Same hook |
| `delete-keeper` + `edit-history` | MessageLogger (partial) | Plugin-side cache of the last 500 messages and the last 25 deletions (`plugins/delete-keeper/README.md:3`). Deleted text shows in a `message-accessory` box with id `deleted-<id>`. If the message row is gone it is rendered as an "orphan" accessory below the timeline (`crates/ui/src/shell/plugins/message.rs:20-21,116-140`; `timeline.rs:837`). Vencord keeps the deleted message **inline** by patching `MessageStore` (§1.3) |
| `word-mute` (`message-filter`) | NoBlockedMessages (same hook) | No stock Vencord word-mute |
| `quick-replies` | CustomCommands (partial) | `/snip` + composer popover |
| `spoiler-all` | inverse of RevealAllSpoilers | |
| `user-notes` | none (Discord has native notes) | `user-display` badge |
| `keyword-highlighter` | none stock | |
| `hello-timestamps`, `local-status-badge`, `conformance-probe` | demos/tests | |

### 3.2 Plugins that need core features first

**[inferred from each plugin's behaviour]**

| Core feature | Plugins it unblocks |
| --- | --- |
| Deleted/edited message retention in the state store (a `deleted` flag and edit history kept per message, rendered inline) | MessageLogger. This upgrades delete-keeper/edit-history |
| Presence write API (activities, status, idle policy) | CustomRPC, IgnoreActivities, CustomIdle, AutoDndWhilePlaying, MusicRichPresence, GameActivityToggle, StreamerModeOnStream |
| Running-game detection | AutoDndWhilePlaying, GameActivityToggle |
| Typing hook (`before-typing`) | SilentTyping |
| Outgoing-message flags (`allowed_mentions`, `SUPPRESS_NOTIFICATIONS`) | NoReplyMention, SilentMessageToggle |
| Upload pipeline hook (`before-upload`) and command-to-attachment output | AnonymiseFileNames, Petpet |
| Reply/user backfill in core (fetch missing referenced messages and users, cached and paced) | ValidReply, ValidUser (these become built-in behaviour) |
| Jump-to-message / history window API | JumpToTop, MessageLinkEmbeds (local hit) |
| Notification pipeline hook (`before-notify`) | OnePingPerDM, XSOverlay, RelationshipNotifier |
| Link-open hook | OpenInApp |
| TTS / audio | VcNarrator, NotificationVolume (voice out of scope) |

### 3.3 Account-ban and privacy flags

33 plugins carry a flag in the table. Each flag gives the risk and how our client should treat it **[inferred, grounded in each plugin's code]**:

- **Fakes or unlocks client features. High risk; never ship, and expose no API that could do it.**
  - **FakeNitro** spoofs premium checks and stream quality. It has 16 patches, overrides `canUseEmojisEverywhere`/`canStreamQuality`, and checks `premiumType` (`fakeNitro/index.tsx:207-265,420-453`) **[read]**.
  - **SpotifyCrack** bypasses Discord's premium listen-along check.
  - **Experiments** unlocks internal experiments and dev UI.
  - The rule: plugins get no write access to super-properties, `premium_type`, experiment buckets or capability flags. These stay core-owned. A plugin that only rewrites `:emoji:` to an image link is just text, but it isn't first-party.
- **Shows what permissions hide (medium).** ShowHiddenChannels (28 patches), ShowHiddenThings and PermissionFreeWill. They are client-only, but they display data or controls the server meant to gate. Treat them as C, with no plugin API to bypass permission checks in our UI.
- **Extra API traffic (medium).**
  - **On render:** MessageLinkEmbeds, WhoReacted (a REST call per reaction, 250 ms queue, `whoReacted/index.tsx:46-82`), ValidUser and ValidReply.
  - **Background polling:** BetterSessions (a `setInterval` on the sessions endpoint, `betterSessions/index.tsx:154,204`) and ImplicitRelationships.
  - **Automation on events:** NewGuildSettings (PATCH on join).
  - Rules for these:
    - Plugins never get raw REST. They get host calls that are cache-first and share one global request budget with the core's pacing (`core.md` §7).
    - On-render fetches are off by default and need an explicit click.
    - Polling is capped at a 15-minute minimum.
- **User-initiated API (low).** ExpressionCloner, FriendInvites, GreetStickerPicker, PauseInvitesForever, ReadAllNotificationsButton and UnsuppressEmbeds. Allowed only inside a user gesture, meaning a UI action or command, with oxidcord-style per-plugin rate limits (`send-message` 1 per 2 s, burst 3; `add-reaction` 1/s, burst 5, `docs/plugin-api/overview.md:33-34`) **[read for oxidcord limits]**.
- **Presence automation (low–medium).** AutoDndWhilePlaying, CustomIdle, CustomRPC, IgnoreActivities and MusicRichPresence, which updates presence every 16 s (`musicRichPresence/index.tsx:313`) **[read]**. Presence writes go through one core presence API with a floor interval and deduplication. Vencord bans "animated status" (`CONTRIBUTING.md:55`) **[read]**, and so do we.
- **Third-party egress (privacy, not ban).** ClearURLs (fetches a rule list from GitHub, `clearURLs/index.ts:25`), Dearrow, Decor, GifProviderSwitcher, MusicRichPresence, ReviewDB, SpotifyControls, Translate and USRBG. They need `net-fetch`, a declared host allowlist shown at install, and https only (as oxidcord, `overview.md:32`). Our first-party ClearURLs should ship its rules in-package.
- **Never, regardless of plugin.** Self-bot behaviour such as auto-reply, message pruning, nitro sniping or animated status (Vencord's own list, `CONTRIBUTING.md:55`; Discord's self-bot policy in `core.md` §7.3).
  - Enforce it structurally: `send-message`, `add-reaction`, REST writes and presence writes are only callable while a user-gesture token is live, and not from event handlers.
  - This is stricter than oxidcord, which allows `send-message` from any hook within rate limits **[inferred design]**.

### 3.4 Full table

"Patches" is the number of `find:` entries in the plugin's files. "oxidcord today" names a first-party oxidcord plugin with the same behaviour.

#### (A) Logic/data only: 25

| Plugin | Patches | What it needs natively | Risk flag | oxidcord today |
| --- | --- | --- | --- | --- |
| anonymiseFileNames | 2 | Pre-upload hook renames attachments | - | - |
| autoDndWhilePlaying.discordDesktop | 0 | Running-game event sets status; needs core game detection + presence write | presence automation (low) | - |
| clearURLs | 0 | before-send rewrite of URLs | third-party (rules list fetched from GitHub) | link-cleaner |
| customCommands | 0 | Local commands/tags + storage | - | quick-replies (partial) |
| customIdle | 1 | Idle timeout policy; needs core idle/presence control | presence automation (low) | - |
| customRPC | 1 | Sets a rich-presence activity; needs core presence API | presence (low) | - |
| fakeNitro | 16 | Pre-send rewrite of emoji/sticker to links (A); also spoofs Nitro checks and stream quality (C) | fakes client features (high) | - |
| friendInvites | 0 | Commands that create/list/revoke friend invites via REST | extra API (user-initiated) | - |
| ignoreActivities | 3 | Filter which activities go into your presence | presence (low) | - |
| keepCurrentChannel | 1 | Remember last channel, navigate on start | - | - |
| musicRichPresence | 0 | Presence from Last.fm/ListenBrainz, refreshed every 16 s | presence automation + third-party | - |
| newGuildSettings | 2 | On guild join, PATCH notification settings automatically | automation (low) | - |
| noBlockedMessages | 3 | filter-message hook for blocked users | - | word-mute (same hook) |
| noReplyMention | 1 | before-send sets allowed_mentions.replied_user=false (needs the field in outgoing-message) | - | - |
| onePingPerDM | 1 | Notification-filter hook | - | - |
| openInApp | 2 | Link-open hook rewriting to app schemes | - | - |
| petpet | 0 | Command that renders a GIF and attaches it (needs command-to-attachment output) | - | - |
| relationshipNotifier | 3 | Relationship/guild-removal events raise a notification; stores a snapshot | - | - |
| silentTyping | 1 | Suppress the typing request (needs a before-typing hook) | - | - |
| spotifyShareCommands | 0 | Commands that send the current Spotify track | - | - |
| streamerModeOnStream | 0 | Stream event toggles streamer mode (voice/stream) | - | - |
| textReplace | 2 | before-send text rules | - | text-shortcuts |
| unindent | 1 | before-send/render trim of code-block indentation | - | - |
| vcNarrator | 0 | Voice events spoken by text-to-speech (voice; needs a TTS host call) | - | - |
| xsOverlay | 0 | Forward notifications to a local VR overlay socket | - | - |

#### (B) UI-surface additions: 53

| Plugin | Patches | What it needs natively | Risk flag | oxidcord today |
| --- | --- | --- | --- | --- |
| accountPanelServerProfile | 1 | Context-menu item on the account panel opening a server-profile view | - | - |
| betterRoleContext | 2 | Context-menu items on roles (copy colour, view icon) | - | - |
| betterSessions | 1 | Settings pane + notifications; polls the sessions endpoint on an interval | extra API (poll) | - |
| blurNsfw | 1 | Attachment render policy in NSFW channels (attachment-render hook or setting) | - | - |
| callTimer | 1 | Voice-panel slot with a timer (voice out of MVP scope) | - | - |
| characterCounter | 2 | Composer slot reading draft length | - | - |
| copyEmojiMarkdown | 0 | Emoji context-menu item | - | - |
| copyFileContents | 1 | Attachment accessory button (needs attachment bytes through the host) | - | - |
| copyStickerLinks | 0 | Sticker context-menu items | - | - |
| copyUserURLs | 0 | User context-menu item | - | - |
| dearrow | 1 | Rewrites YouTube embed title/thumbnail; embed-render hook + fetch | third-party | - |
| decor | 6 | Avatar decorations from a third-party server; avatar-decoration hook | third-party | - |
| expressionCloner | 0 | Context-menu item that uploads an emoji/sticker to your server via REST | extra API (user-initiated) | - |
| fakeProfileThemes | 2 | Decodes invisible 3y3 colours in bios; profile-render hook | fakes client features (low) | - |
| forceOwnerCrown | 1 | Member-list/name decoration (owner crown) | - | - |
| gameActivityToggle | 1 | Button in the account panel that writes a user setting | - | - |
| greetStickerPicker | 2 | Context menu on the wave button; sends a sticker | extra API (user-initiated) | - |
| hideAttachments | 1 | Message-popover button + per-message render flag + storage | - | - |
| implicitRelationships | 9 | Friends-tab section using the affinities endpoint | extra API | - |
| ircColors | 2 | Name-colour override (display-user hook) | - | - |
| jumpToTop | 1 | Header button that jumps to the first message (needs a jump-to API) | - | - |
| memberCount | 3 | Member-list header slot from member-count data | - | - |
| messageClickActions | 0 | Message click hook (double-click edit/reply, backspace-click delete) | - | - |
| messageLatency | 1 | Message decoration from nonce vs server timestamp | - | - |
| messageLinkEmbeds | 1 | Accessory that fetches linked messages via REST (queued) | extra API (on render) | - |
| mutualGroupDMs | 3 | Profile section listing mutual group DMs | - | - |
| oneko | 0 | Canvas overlay following the pointer | - | - |
| pauseInvitesForever | 1 | Guild-settings button that PATCHes invite pause | extra API (user-initiated) | - |
| permissionsViewer | 1 | Context menu + panel computed from roles/overwrites in state | - | - |
| platformIndicators | 3 | Badge/decoration from presence client_status | - | - |
| previewMessage | 0 | Composer button rendering a local preview | - | - |
| quickMention | 0 | Message-popover button inserting a mention into the composer | - | - |
| readAllNotificationsButton | 0 | Server-list button; bulk read-state ack via REST | extra API (user-initiated) | - |
| replaceGoogleSearch | 0 | Text-selection context menu | - | - |
| reverseImageSearch | 1 | Image context-menu item opening a URL | - | - |
| reviewDB | 2 | Profile section backed by a third-party review service with OAuth | third-party | - |
| sendTimestamps | 0 | Composer button + before-send rewrite to timestamp markup | - | - |
| serverInfo | 0 | Guild context-menu item + info panel | - | - |
| serverListIndicators | 0 | Guild-rail slot showing counts | - | - |
| showConnections | 1 | Profile section of connected accounts | - | - |
| showMeYourName | 1 | Name override (display-user hook) | - | - |
| showTimeoutDuration | 1 | Decoration with timeout remaining | - | - |
| silentMessageToggle | 0 | Composer button + before-send sets SUPPRESS_NOTIFICATIONS flag | - | - |
| spotifyControls | 3 | Player panel driving Spotify's Web API with the Discord-issued Spotify token | third-party | - |
| translate | 0 | Popover/composer buttons + accessory; sends text to Google/DeepL/Kagi | third-party | - |
| typingIndicator | 2 | Channel-list decoration from typing state | - | - |
| unsuppressEmbeds | 0 | Context-menu item PATCHing message flags | extra API (user-initiated) | - |
| userMessagesPronouns | 2 | Decoration from profile pronouns | - | - |
| userVoiceShow | 4 | Decoration from voice-state data | - | - |
| usrbg | 3 | Profile banner from a third-party database | third-party | - |
| viewIcons | 5 | Context-menu items opening icons/banners in a viewer | - | - |
| viewRaw | 0 | Popover/context menu showing raw message JSON | - | - |
| whoReacted | 3 | Reaction decoration; fetches reactors per message via REST (250 ms queue) | extra API (on render) | - |

#### (C) Deep UI patches, become native built-ins: 58

| Plugin | Patches | What it needs natively | Risk flag | oxidcord today |
| --- | --- | --- | --- | --- |
| addAttachments | 0 | Attach files while editing: native edit flow must support attachment upload | - | - |
| alwaysAnimate | 6 | Built-in setting: animate avatars/emoji/banners always | - | - |
| alwaysExpandRoles | 1 | Built-in setting in profile view | - | - |
| alwaysTrust | 2 | Built-in setting: skip untrusted-link/suspicious-file prompts (if we have them) | - | - |
| betterFolders | 5 | Server-folder sidebar is a layout feature of the guild rail | - | - |
| betterGifAltText | 2 | Accessibility text for GIFs: native a11y layer | - | - |
| betterGifPicker | 1 | GIF-picker default tab | - | - |
| betterRoleDot | 4 | Role-dot vs coloured-name rendering setting | - | - |
| biggerStreamPreview | 0 | Stream-preview context item; voice/stream is out of MVP scope | - | - |
| clientTheme | 0 | Accent colour for the client: a theme token, not a plugin | - | - |
| colorSighted | 2 | Status-icon rendering setting | - | - |
| disableCallIdle | 2 | Client-side voice idle kick timer (voice out of scope) | - | - |
| dontRoundMyTimestamps | 0 | Timestamp-format setting | - | - |
| favEmojiFirst | 2 | Emoji autocomplete ordering | - | - |
| fixImagesQuality | 1 | Media-loading policy (original resolution) | - | - |
| fullSearchContext | 1 | Context menu in search results: native search UI | - | - |
| fullUserInChatbox | 1 | Composer mention-chip interactivity | - | - |
| gifPaste | 1 | GIF-picker insert-vs-send behaviour | - | - |
| gifProviderSwitcher | 5 | GIF-picker provider (Tenor/Giphy/Klipy) | third-party | - |
| iLoveSpam | 1 | Spam-hiding rule: our client decides what is hidden | - | - |
| imageFilename | 1 | Image tooltip | - | - |
| imageLink | 1 | Rendering rule for lone image links | - | - |
| imageZoom | 3 | Media-viewer lens zoom | - | - |
| mentionAvatars | 2 | Mention-pill rendering | - | - |
| messageLogger | 12 | Needs core retention of deleted/edited messages and inline rendering | - | delete-keeper + edit-history (partial) |
| moreQuickReactions | 1 | Hover-bar reaction count | - | - |
| noMaskedUrlPaste | 1 | Composer paste behaviour | - | - |
| noMosaic | 3 | Attachment-layout setting | - | - |
| noPendingCount | 4 | Badge-count rules | - | - |
| noProfileThemes | 1 | Profile-theme rendering setting | - | - |
| noServerEmojis | 1 | Emoji-autocomplete filter | - | - |
| noSystemBadge.discordDesktop | 1 | Taskbar/tray badge setting | - | - |
| noTypingAnimation | 1 | Typing-dots animation setting | - | - |
| noUnblockToJump | 1 | Jump-to-message rule for blocked users | - | - |
| notificationVolume | 1 | Notification-sound volume (audio output) | - | - |
| overrideForumDefaults | 1 | Forum layout defaults | - | - |
| pictureInPicture | 1 | Video player PiP | - | - |
| pinDms | 4 | DM-list pin/order (list-ordering feature) + context menu | - | - |
| plainFolderIcon | 1 | Guild-folder rendering setting | - | - |
| quickReply | 0 | Keybinds to walk messages for reply/edit | - | - |
| replyTimestamp | 1 | Reply-preview rendering | - | - |
| revealAllSpoilers | 1 | Spoiler interaction | - | spoiler-all (inverse) |
| roleColorEverywhere | 8 | Role colours across mentions/lists/voice/reactions | - | - |
| shikiCodeblocks.desktop | 2 | Code-block syntax highlighting: native renderer feature | - | - |
| showAllMessageButtons | 1 | Hover-bar behaviour | - | - |
| showHiddenChannels | 28 | Lists channels you lack VIEW_CHANNEL for (28 patches) | fakes client features (medium) | - |
| showHiddenThings | 5 | Shows moderator-only UI regardless of permissions | fakes client features (medium) | - |
| sortFriendRequests | 2 | Friends-list ordering | - | - |
| stickerPaste | 1 | Sticker-picker insert-vs-send | - | - |
| superReactionTweaks | 2 | Super-reaction playback limit | - | - |
| typingTweaks | 1 | Typing-indicator rendering | - | - |
| unlockedAvatarZoom | 1 | Avatar crop tool | - | - |
| validReply | 2 | Fetch missing reply references: core message fetching | extra API | - |
| validUser | 2 | Fetch unknown users for mentions: core user cache | extra API | - |
| vcDoubleClick | 2 | Voice-channel join gesture (voice) | - | - |
| voiceDownload | 1 | Voice-message attachment download | - | - |
| voiceMessages | 1 | Record and send voice messages (audio capture + waveform upload) | - | - |
| volumeBooster | 5 | Per-user volume above the default maximum (voice) | - | - |

#### (D) Irrelevant to a native client: 32

| Plugin | Patches | What it needs natively | Risk flag | oxidcord today |
| --- | --- | --- | --- | --- |
| arRPC.web | 0 | Web-only RPC bridge; native equivalent is a core local RPC/IPC server | - | - |
| betterSettings | 6 | Speeds up Discord's own settings modal | - | - |
| betterUploadButton | 1 | Discord upload-button click semantics; our composer defines its own | - | - |
| consoleJanitor | 11 | Silences Discord's devtools console | - | - |
| consoleShortcuts | 1 | Devtools console aliases | - | - |
| crashHandler | 1 | Recovers Discord's React tree after crashes | - | - |
| devCompanion.dev | 1 | Dev tool for writing webpack patches | - | - |
| directFileDownload.desktop | 2 | Electron download behaviour; a native client downloads natively | - | - |
| experiments | 7 | Unlocks Discord's internal experiment/dev UI | fakes client features (high) | - |
| f8break | 0 | Devtools debugger shortcut | - | - |
| fixCodeblockGap | 1 | Discord CSS quirk | - | - |
| fixSpotifyEmbeds.desktop | 0 | Spotify iframe volume in Electron | - | - |
| fixYoutubeEmbeds.desktop | 0 | YouTube iframe blocking in Electron | - | - |
| loadingQuotes | 1 | Discord's loading-screen quotes | - | - |
| noDeepLinks.web | 1 | Web deep-link prompt | - | - |
| noDevtoolsWarning | 1 | Devtools self-XSS banner | - | - |
| noF1 | 1 | Electron F1 help shortcut | - | - |
| noMiddleClickPaste | 0 | Chromium/Linux middle-click paste quirk | - | - |
| noOnboardingDelay | 1 | Discord web onboarding delay | - | - |
| permissionFreeWill | 2 | Lifts Discord's client-side permission-editor guards | fakes client features (medium) | - |
| reactErrorDecoder | 1 | React error messages | - | - |
| secretRingTone | 1 | Ringtone easter egg (voice) | - | - |
| spotifyCrack | 2 | Bypasses Discord's Spotify-premium listen-along check | fakes client features (high) | - |
| startupTimings | 0 | Discord startup profiling | - | - |
| themeAttributes | 4 | Adds DOM data-attributes for CSS themes | - | - |
| vencordToolbox | 1 | Vencord's own quick-actions menu (our plugin manager replaces it) | - | - |
| webContextMenus.web | 12 | Restores context menus missing on web | - | - |
| webKeybinds.web | 2 | Restores keybinds missing on web | - | - |
| webPWA.browser | 3 | Makes the web app installable | - | - |
| webScreenShare.browser | 1 | Browser screen-share options | - | - |
| webScreenShareFixes.web | 2 | Chromium screen-share bitrate fix | - | - |
| youtubeAdblock.desktop | 0 | AdGuard in YouTube embeds/WatchTogether | - | - |


---

## 4. Designing a "Vencord-like" system

### 4.1 The two candidate runtimes

| | oxidcord WASM components (WIT) | zgui Luau plugins |
| --- | --- | --- |
| Where it is | `crates/plugin-host` (wasmtime 50, component model; `Cargo.toml:87-89`). UI-agnostic: no gpui dependency (`crates/plugin-host/Cargo.toml` deps) **[read]**. oxidcord's UI that renders slots is gpui (`Cargo.toml:115`; `crates/ui/src/shell/plugins/*`) **[read]**, so we'd rewrite that half for zgui | Built into zgui-script (`mlua` 0.12 with `luau`, `crates/zgui-script/Cargo.toml`) **[read]** |
| Authoring | Rust crate → `wasm32-wasip2` component (`docs/plugin-api/building.md:40-45`) **[read]**. JS possible via ComponentizeJS (SpiderMonkey; "The total embedding size is around 8MB", https://github.com/bytecodealliance/ComponentizeJS) **[read]** | Luau with luau-lsp types, `zgui check`/`zgui lint` (`docs/scripting.md:44-93`) **[read]**. Same language and components as our app UI |
| Hot reload | `PluginManager::reload(id)` reloads from disk (`crates/plugin-host/src/manager.rs:308-313`). No file watcher; a rebuild is needed **[read]** | Built in: `host.watch`, state kept across reloads, last good UI kept on error (`docs/scripting.md:17-24`). Managed plugins are "watched for edits" (`:318`) **[read]** |
| Sandbox | One instance per plugin on its own thread, epoch interruption every 10 ms, 64 MB linear memory, WASI with an empty context, crash restart with backoff, 5 crashes/60 s disables (`overview.md:81-87`; `runtime.rs:54-59`; `config.rs:22-65`) **[read]** | A VM per plugin, read-only globals, 32 MiB heap, 50 ms CPU per call (`docs/scripting.md:24`). Luau is designed "safe to embed" for untrusted code, with VM interrupts (https://luau.org/sandbox/) **[read]** |
| Permissions | 19 fine-grained permissions (`wit/oxidcord.wit:182-202`) with per-call checks, rate limits, a host allowlist for fetch, consent reasons per permission in the manifest (`plugins/delete-keeper/plugin.toml:11-21`) and signed catalogs (ed25519, `catalog.rs:86-96`) **[read]** | Coarse built-ins (`Capability::{Fetch, Clipboard, Files, Terminals, Web, Windows}`) plus app-defined `Named(String)` on `expose_action`/`expose_shared` (`crates/zgui-script/src/lib.rs:611-629,1177-1190`) **[read]**. `expose_shared` grants **read and write** (`lib.rs:1183`) **[read]**. Fetch is all-or-nothing unless the app's fetcher enforces hosts (`lib.rs:1130`) **[inferred]** |
| UI contribution | Declarative `ui-tree` of 12 node kinds (column, row, text, button, input, toggle, image, badge, divider, icon, progress, link; `wit:167-180`), max 2,000 nodes (`overview.md:75`). Slots: `message-action`, `message-accessory`, `composer-toolbar`, `title-bar`, `status-bar`, `sidebar-panel`, `right-panel`, `settings-page`, `modal`, `popover`, `toast`, and `server-`/`channel-`/`user-`/`message-context-menu` (`crates/ui/src/shell/**`) **[read]** | Real zgui components via `ui.slot(name, {context})` / `ui.contribute(name, node)`: any element, animation, canvas, state (`docs/scripting.md:316`) **[read]**. Context menus are a node property `context_menu = function(e) return {…} end` (`:176`) **[read]**, so plugin menu items need an app-side merge of slot contributions **[inferred]** |
| Performance | Hooks run off the UI thread. Render results are cached per message revision: plain render first, plugin AST later (`crates/session/src/render.rs:154-166`, `task/plugin_link.rs:134-170`) **[read]**. Hook deadlines: render 50 ms, send 500 ms, events 200 ms (`config.rs:22-30`) **[read]**. Cost: wasmtime + cranelift in the binary **[inferred: several MB; not measured]** | Luau vs Rust in zgui's streaming test: "about 2% more instructions … and 0.9 MiB more memory" (`docs/scripting.md:495`) **[read]**. But `ScriptHost` is single-threaded (`RefCell` state, `lib.rs:1158`) **[read]**, so plugin scripts run on the UI thread. A per-message hook there competes with frames **[inferred]** |
| Reaching Discord data | Typed events (9 kinds, `wit:280-290`), typed queries (`current-user`, `channel-messages` ≤100, `list-guilds/channels/members`, `display-name`, `voice-state`), hooks (`render-message` on a markdown AST, `before-send`, `filter-message`, `display-user`), actions (`send-message`, `add-reaction`, `open-channel`, `notify`, `clipboard-write`, `open-url`), storage (5 MB/1024 keys), fetch (`wit:305-345`; `overview.md:27-45`) **[read]** | Whatever the app exposes as shared state and actions. Reactive by default: `ui.shared` keys are tracked state, so a plugin re-renders when published data changes (`docs/scripting.md:293-296`) **[read]** |

**Takeaways** **[inferred]**:
- **oxidcord's host is the better policy engine:** permissions, limits, conformance, off-thread hooks and render caching.
- **zgui's Luau is the better authoring and UI engine:** hot reload, real components, same language as the app, and no second compiler toolchain for plugin authors.
- The weak spots are mirror images. WASM UI is a tiny declarative tree, and we'd have to write a new zgui renderer for it anyway. Luau plugins in zgui run on the UI thread with coarse capabilities.

### 4.2 Recommended architecture: one contract, Luau in two VMs

```
            ┌──────────── Rust core (state, gateway, REST pacing) ────────────┐
            │  Hook bus: events · queries · hooks · actions · storage · fetch │
            │  Policy: permissions · user-gesture tokens · rate/budget · deadlines
            └──────────────┬───────────────────────────────┬──────────────────┘
                           │ typed calls (worker thread)   │ expose_shared / expose_action
             plugin/logic.luau (mlua Luau VM, sandbox,     │ (Capability::Named("discord.*"))
             memory limit, interrupt deadline)             │
                           │  plugin-local channel          ▼
                           └──────────────►  plugin/ui.luau (zgui plugin VM → ui.contribute)
```

- **Contract.** Write the API as a versioned IDL. oxidcord's WIT is a good starting point to copy *in design*; it is MIT/Apache, so even copying it is fine. Generate Luau type definitions from it, so authors get completion and strict checking **[inferred]**.
- **Logic VM (worker thread).**
  - Create the VM with `mlua` (already linked by zgui), using `Lua::sandbox(true)`, `set_memory_limit` and `set_interrupt` for deadlines (https://docs.rs/mlua/0.12.2/mlua/struct.Lua.html: `sandbox` and `set_interrupt` are Luau-only) **[read]**.
  - Port oxidcord's runtime policy unchanged: per-hook deadlines, crash backoff, queue of 256 events with interactive calls first, render cache keyed by message revision (`overview.md:81-87`; `render.rs`) **[read for oxidcord; inferred for the port]**.
- **UI VM.**
  - `ScriptHost::set_plugins(vec![Plugin{ name, entry: ui.luau, grants }])`.
  - Discord data is published as `ui.shared` keys such as `discord.selected_channel`, `discord.typing.<channel>` and `discord.plugin.<id>.state`. Each is exposed with `Capability::Named` matching the contract's permissions.
  - Actions (`discord.send`, `discord.react`, `discord.open_channel`, `discord.copy`) are exposed with the same capability names.
  - Because `expose_shared` is read-write (`lib.rs:1183`), the app must treat plugin writes to `discord.*` keys as ignored. The alternative is a small zgui change for read-only exposure **[inferred]**.
- **Why not WASM first.** It costs a second engine (wasmtime + cranelift) in a client whose selling point is lightness. It also means Rust-only authoring, a new zgui renderer for oxidcord's `ui-tree` anyway, and no hot reload of UI **[inferred]**.
- **What we take from oxidcord now:** its permission set, limits, conformance runner, catalog signing and packaging (`package.rs`, `catalog.rs`). Keep `plugin-host` as the second backend for later; the same contract can run either **[inferred]**.

### 4.3 Hook surface, mapped from Vencord

**[inferred design; oxidcord counterparts read]**

| Vencord API | Our hook / slot | Thread | Permission | oxidcord counterpart |
| --- | --- | --- | --- | --- |
| `onBeforeMessageSend` / `onBeforeMessageEdit` | `before_send(msg) -> pass \| replace \| cancel` and `before_edit`. The message includes `flags` and `allowed_mentions`. The UI marks rewritten messages | logic | `message.send.rewrite` | `before-send` (no edit, no flags) |
| (patches on message render) | `render_message(view, md_ast) -> ast?`, cached per revision; `filter_message(view) -> show \| hide \| collapse` | logic | `message.render`, `message.filter` | `render-message`, `filter-message` |
| MessageAccessories | slot `message.accessory`, context `{message_id, channel_id}` | UI | `ui.slots` + `context.timeline` | `message-accessory` |
| MessageDecorations / MemberListDecorators / Badges | `display_user(guild, user) -> {name?, color?, badges[]}` (logic) + slots `message.author.decoration`, `member.decoration`, `profile.badge` (UI) | both | `user.display` | `display-user` (one badge) |
| MessagePopover | slot `message.action` | UI | `ui.slots` | `message-action` |
| ContextMenu (`navId`) | slots `menu.message`, `menu.user`, `menu.channel`, `menu.guild`, `menu.image`, `menu.link`, `menu.text-selection`. Items are `{id, label, icon, submenu?}`; the app merges them into its `context_menu` functions | UI | `ui.slots` | `*-context-menu` slots |
| ChatButtons | slot `composer.toolbar`, plus actions `composer.insert(text)` and `composer.get_draft()` | UI | `composer.read/write` | `composer-toolbar` (no draft access) |
| ServerList | slot `guild_rail.top/bottom` | UI | `ui.slots` | `sidebar-panel` (approximate) |
| Notices / Notifications | `notify(title, body)`, `toast(text)`, slot `banner` | either | `actions.notify` | `notify`, `toast` slot |
| Commands | `register_command{name, options[] typed}` → `on_command(args, ctx)`. Local only, never sent to Discord | logic | `commands` | `register-command` (string args) |
| `flux` handlers | `on_event(e)` with a typed, curated event list: message create/update/delete/bulk-delete, reaction add/remove, typing, channel select, presence, relationship add/remove, guild join/leave, voice state, connection, settings. No raw gateway passthrough | logic | `context.*` per event | 9 events |
| Store queries (`findStore`) | Typed queries: `channel`, `guild`, `member`, `user`, `messages(channel, limit)`, `presence`, `relationships`, `read_state`. Read-only snapshots from core state | logic | `context.*` | `list-*`, `channel-messages` |
| `RestAPI` | **No raw REST.** Named actions only (`react`, `pin`, `edit_own`, `delete_own`, `set_presence`, `ack`), each paced by the core and gesture-gated where it writes | logic | per action | `send-message`, `add-reaction` |
| Settings (`OptionType`) | Declarative schema (string, number, bool, select, slider, list, keybind) rendered by our settings page; `on_setting_changed` | logic | none | `settings-schema` (JSON) |
| DataStore | `storage.get/set/delete/list`, per plugin, quota | logic | `storage` | same (5 MB/1024 keys) |
| Styles / `managedStyle` | Plugin-declared app tokens (`ui.theme.tokens`) only. No arbitrary styling of app surfaces | UI | none | theme in plugin package |
| (new) typing, upload, notify, link-open | `before_typing(channel) -> allow \| suppress`, `before_upload(files) -> renamed`, `before_notify(n) -> allow \| suppress`, `on_link_open(url) -> url \| app-scheme` | logic | per hook | none |

The **flux-like event bus** is the core's own event stream, which already drives state (`core.md`). Plugins subscribe by name, and the host fans out the events the plugin's permissions allow, in the same order the state store applied them **[inferred]**.

### 4.4 Is a TS/JS authoring path worth it?

**Not for v1. Revisit once there is a third-party ecosystem to serve.**

- **Vencord code doesn't port anyway.** Familiarity with TS buys little: the hard parts of a Vencord plugin are Discord-internal regexes and React, which have no equivalent here (§2) **[inferred]**.
- **QuickJS or SpiderMonkey runtime.**
  - ComponentizeJS embeds SpiderMonkey at "around 8MB" (read) per component, probably per plugin instance **[inferred]**.
  - QuickJS (for example `rquickjs`) is smaller, but it is a third language runtime next to Luau and Rust, with its own sandbox to audit **[inferred]**.
- **JS → Luau.** roblox-ts compiles TypeScript "into Luau code for use inside of Roblox", with Roblox typings (https://roblox-ts.com/docs/) **[read]**. Re-targeting it to our API would mean maintaining a fork of a compiler and its runtime library **[inferred]**.
- **JS → WASM (ComponentizeJS against our WIT).** This is the cleanest path *if* we adopt the WASM backend later, since the same contract serves both. It is the right long-term answer for JS authors **[inferred]**.
- **What we do instead.** Ship Luau types and a `definePlugin`-shaped table (`return plugin { name, description, settings, flux = {…}, before_send = …, commands = {…} }`), so Vencord authors recognise the structure **[inferred]**.

### 4.5 Theming

**What carries over from Vencord/BetterDiscord themes** **[inferred, from §1.5]**:
- Only values. Discord themes override CSS custom properties (Discord's colour variables) and then restyle hashed class selectors.
- Our importer should parse the CSS and keep only custom-property declarations in `:root` / `.theme-dark` / `.theme-light` / `.visual-refresh` blocks. It maps known Discord variable names to our tokens and reports the rest as dropped.
- oxidcord's token names already mirror Discord's variables (`background_primary`, `text_normal`, `interactive_hover`, `mention_background`, `status_dnd`, …; `docs/themes.md:32-56`) **[read]**, so that mapping table is mostly written.

**What can't carry over** **[inferred]**:
- class and attribute selectors;
- layout changes and element hiding;
- pseudo-elements and animations;
- background images in arbitrary places;
- `@import` of remote CSS;
- font-face loading from URLs.

**Native theme format.** Use zgui's theme JSON as is: `name`, `type`, `extends`, nested `tokens`, references and `alpha/mix/over` derivations, hot-reloaded (`docs/styling.md:398-450`) **[read]**. Add a `discord.*` app-token namespace declared with defaults in standard tokens (`ui.theme.tokens`, `docs/scripting.md:407`; `styling.md:452-460`) **[read]**:

| Group | Tokens |
| --- | --- |
| Chrome | `discord.rail.bg`, `discord.sidebar.bg`, `discord.chat.bg`, `discord.members.bg`, `discord.titlebar.bg` |
| Messages | `discord.message.hover`, `discord.message.mentioned.bg`, `discord.message.mentioned.bar`, `discord.message.deleted.bg`, `discord.message.edited.text` (for logger plugins) |
| Mentions and markup | `discord.mention.bg/text`, `discord.spoiler.bg/revealed`, `discord.code.bg/border`, `discord.blockquote.bar`, `discord.link` |
| Status and indicators | `discord.status.online/idle/dnd/offline/streaming`, `discord.unread.pill`, `discord.badge.mention`, `discord.typing.dots`, `discord.speaking.ring` |
| Density | `discord.message.avatar`, `discord.message.gutter`, `discord.row.channel`, `discord.row.member`, `discord.rail.width`, `discord.sidebar.width`, `discord.members.width` (as oxidcord's size tokens, `themes.md:44-56`) |
| Type | Standard `font.sans`, `font.mono`, `font.size.*` |

- **Rules:** a theme is data only and can't run code (oxidcord's rule, `themes.md:3`) **[read]**.
- **QuickCSS equivalent:** a live "Quick Theme" JSON file in the themes folder, which zgui hot-reloads in place (`styling.md:427-430`) **[read]**.
- **Plugins** can declare their own `plugin.<id>.*` tokens with defaults, so any theme styles them **[inferred]**.

---

## 5. Recommendation and first plugins

**Answer to "support Vencord?": partial.**
- **Vencord's code and themes: no,** short of turning the app into a web-view Discord client (Vesktop), which defeats the point and has the same ToS exposure.
- **Vencord's ideas: yes.** About half of its plugins (78 of 168, A+B) map onto a hook-and-slot API. Another third (58, C) become built-in features or settings of our own UI, many of them one toggle each. The rest (32, D) are web or Electron artefacts.
- **The plugin system:** the §4.2 design. One versioned contract, Luau in a worker-thread logic VM plus a zgui plugin VM for UI, and oxidcord's policy model (permissions, gesture-gated writes, rate limits, deadlines, signed catalog, conformance). WASM is a later optional backend for Rust/JS authors.

**First plugins, ordered by user value against cost** **[inferred]**:

| # | Plugin (ours) | Vencord equivalents | Needs |
| --- | --- | --- | --- |
| 1 | Message Logger | MessageLogger | Core retention of deleted/edited messages (inline, a cap per channel, purge on logout) + `message.accessory`/render hook. Upgrades oxidcord's delete-keeper/edit-history |
| 2 | Clear URLs | ClearURLs | `before_send`/`before_edit`; rules bundled in the package (no fetch) |
| 3 | Silent Typing | SilentTyping | `before_typing` hook + composer toggle |
| 4 | Text Replace | TextReplace, Unindent | `before_send` + settings list |
| 5 | Reply & Silent controls | NoReplyMention, SilentMessageToggle | `allowed_mentions` and `flags` in the outgoing message + composer toggle |
| 6 | Names & Colours | ShowMeYourName, IrcColors, RoleColorEverywhere (subset) | `display_user` hook |
| 7 | Indicators | PlatformIndicators, TypingIndicator, UserVoiceShow | presence/typing/voice-state queries + decoration slots |
| 8 | Translate | Translate | `message.action` slot + accessory + `net-fetch` with a host allowlist (user picks the provider, consent at install) |
| 9 | Copy & Inspect | ViewRaw, CopyUserURLs, CopyEmojiMarkdown, ReverseImageSearch, PermissionsViewer | context-menu slots + clipboard + `open_url` + a read-only permissions query |
| 10 | Message Link Embeds | MessageLinkEmbeds | cache-first lookup in local state. A REST fetch only on click, paced by the core |

Build the core prerequisites in this order: deleted/edited retention, outgoing flags, typing hook, context-menu slot merge, then presence write (which unblocks the CustomRPC family later) **[inferred]**.

---

## Sources

**Repositories (read-only clones):**
- Vencord @ `443688b5366a4d43e9868519f48a4544cd7520d4`: https://github.com/Vendicated/Vencord. Files cited inline: `src/main/patcher.ts`, `src/preload.ts`, `src/main/index.ts`, `src/main/persistAfterDiscordUpdates.ts`, `browser/manifest.json`, `browser/modifyResponseHeaders.json`, `src/webpack/patchWebpack.ts`, `src/webpack/webpack.ts`, `src/webpack/common/stores.ts`, `src/utils/types.ts`, `src/api/*`, `src/plugins/_api/*`, `src/plugins/_core/noTrack.ts`, `src/plugins/*/index.ts(x)`, `scripts/build/common.mjs`, `scripts/runInstaller.mjs`, `packages/discord-types/src/fluxEvents.d.ts`, `README.md`, `CONTRIBUTING.md`, `LICENSE`, `package.json`.
- Vesktop @ `3cb2a7bc1306923b60157231430c5b4e78e531da`: https://github.com/Vencord/Vesktop. Files: `src/main/mainWindow.ts`, `src/preload/index.ts`, `src/main/constants.ts`, `package.json`.
- oxidcord @ `497958b`: `crates/plugin-host/wit/oxidcord.wit`, `crates/plugin-host/src/{runtime,config,manager,slots,catalog,package}.rs`, `crates/plugin-host/Cargo.toml`, `crates/session/src/render.rs`, `crates/session/src/task/plugin_link.rs`, `crates/ui/src/shell/plugins/*.rs`, `crates/ui/src/shell/timeline.rs`, `docs/plugin-api/{overview,building}.md`, `docs/themes.md`, `plugins/*/plugin.toml`, `plugins/README.md`, `plugins/delete-keeper/README.md`, `Cargo.toml`.
- zgui @ `a9dc282`: `docs/scripting.md`, `docs/styling.md`, `docs/windows.md`, `crates/zgui-script/src/lib.rs`, `crates/zgui-script/Cargo.toml`, `crates/zgui/src/web_view.rs`, `crates/zgui-desktop/src/native_host/webview2.rs`, `crates/zgui-desktop/examples/terminal-plugins/notes/*`, `crates/zgui-desktop/examples/terminal/plugins.rs`.

**Web:**
- Vencord custom plugins doc: https://docs.vencord.dev/installing/custom-plugins/
- GNU GPL FAQ (#GPLPlugins, #GPLAndPlugins, #NFUseGPLPlugins, #TranslateCode): https://www.gnu.org/licenses/gpl-faq.html
- Discord Terms of Service and self-bot policy, quoted in `core.md` §7.3: https://discord.com/terms, https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots
- Luau sandboxing: https://luau.org/sandbox/
- mlua 0.12.2 `Lua` (`sandbox`, `set_memory_limit`, `set_interrupt`): https://docs.rs/mlua/0.12.2/mlua/struct.Lua.html
- ComponentizeJS: https://github.com/bytecodealliance/ComponentizeJS
- roblox-ts: https://roblox-ts.com/docs/

**Earlier note:** `core.md` (this folder), for the ToS text, self-bot policy, account-risk mitigations and zgui memory baselines.
