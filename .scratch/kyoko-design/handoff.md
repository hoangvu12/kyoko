# kyoko design chat handoff

This is the design chat, not the ticket orchestrator. The v1 tickets in `.scratch/kyoko-v1/issues/` are run by an orchestrator on another PC that pushes to `main`; don't claim, edit or implement them here. Design work lives on the `design` branch of the main checkout (`C:\Users\ADMIN\Desktop\nguyenvu\kyoko`), rebased onto `origin/main` often. Nothing on `design` is pushed yet.

Read first: `AGENTS.md`, `docs/ui.md`, then `docs/design/midnight.md` (the target spec).

## The decision

Kyoko's look moves from a 1:1 port of oxidcord to a 1:1 of Discord's current desktop client, styled with refact0r's Midnight theme, Vencord flavour (`midnight-vencord.theme.css` 2.1.1, MIT). BetterDiscord and Vencord themes can't run in kyoko (they are CSS against Discord's DOM); only their variable block can map onto kyoko tokens, which fits ticket 57's import. Vencord plugin UI is out of scope: the Spotify player, the member counts at the top of the member list and the composer's translate button.

## What is on `design`

- `docs/design/midnight.md`: the spec. Palette resolved to hex, the floating-panel structure, every region, settings, motion, measured sizes, the mockups and their caveats.
- `docs/design/mockups/`: 60 pages of plain HTML and CSS at 1280 by 800, every value a Midnight variable in one `:root` block, no scripts. Main screen; settings (My Account, Notifications, Appearance, Plugins, Themes); server settings (Overview, Roles, Emoji, Stickers, Members, Invites, Bans, Audit Log); profile popout and full profile; status menu; message menu (others' and own); emoji, GIF and sticker pickers; lightbox; inbox; pins; threads (list, split view, create); forum (channel, post, new post); Home/Friends (Online, All, Pending, Blocked, Add Friend); DM and group DM; quick switcher; search results; toast; connection lost; eight dialogs; login in nine states covering all four sign-in methods (email and password with 2FA, QR, web login, token) as the spec and tickets 21, 42, 43 and 44 require.
- `docs/design/reference/midnight-vencord.png`: the user's screenshot of Midnight.
- Uncommitted on purpose and listed in `.git/info/exclude`: `docs/design/reference/live-*.png`, captures of the user's own Discord (settings pages, the profile editor, the main view). They show the user's account details and other people's messages. Use them for your own comparison only; never commit them or upload them anywhere.

## Settled during review

- Shell measured on the live Windows client at 100%: 12-point gaps, rail 66 with 40-point rounded-square server icons, header 50, composer 47, channel rows 36 high and 38 apart, member rows 44 apart. Channel sidebar 280 and member list 264 in the mockups (Discord lets the user resize them; the user's are 306 and 269).
- Settings are a rounded dialog over the dimmed app with Discord's current sidebar groups; kyoko's plugin and theme pages take the place of Vencord's.
- Message menu: Discord's look and order with kyoko's actions only (no Apps or Speak Message; kyoko's Select Message kept). When porting, `bridge::message_menu` changes to this order.
- Login wording follows `docs/sign-in.md`. Toasts are overlays and never move the timeline.

## Expressions and commands (second design chat)

Done on `design`: 37 more pages (97 in all), all checked at 1280 by 800, and the "Expressions and commands" section of `docs/design/midnight.md`. Briefs sent to Claude Design are kept beside this file (`brief-*.md`).

- Pickers, measured on the live client: one 498 by 486 popover anchored 8 above the composer and 35 in from its right edge, never centred. GIF results, emoji search, no-Nitro (greyed and locked, no upsell), emoji autocomplete, reactions with super reactions and the list dialog, expressions in messages with their popouts.
- Slash commands: picker, options, choices and autocomplete states, file options, reply states (sending, thinking, ephemeral, did not respond, interaction failed), components (buttons, selects, Components V2), the app modal, the apps launcher (498 wide, no Promoted activities).
- Composer extras: live markdown in the field, uploads tray, drop overlay, upload failure, + menu, poll creation and polls, voice message playback, a sent link's embed.
- Decided by the user: Unicode emoji are Twemoji (`jdecked/twemoji`, CC-BY 4.0) everywhere, bundled; every mockup uses them. The gift button is left out (Discord shows no send button either).
- Captures from this chat (ignored, never commit): `live-gif-picker`, `live-gif-results-hover`, `live-sticker-picker`, `live-emoji-picker*`, `live-command-picker`, `live-command-options`, `live-apps-launcher`, `live-message-menu-hover`.
- Not captured (running a command or sending would act on the account): command replies, ephemeral messages, components; those follow Discord from memory.
- To raise with the orchestrator: Discord's GIF provider is now Klipy ("Search Klipy"), but `crates/rest/src/api/expressions.rs` sends `provider=tenor`. Built-in commands (`/shrug`, `/tableflip`, `/me`, `/spoiler`) show in the picker mockups but kyoko does not implement them yet; recommended: add the text-only ones.
- Download helper: the project's `ListFiles` endpoint (POST `/design/anthropic.omelette.api.v1alpha.OmeletteService/ListFiles` with `{projectId, path: "export"}` from the design tab) lists every exported page.

## Open items

- Settled: menu item hover is the neutral `--hover` wash (`#373534` on the menu's `#31302f`, measured).
- Pages without a live capture behind them follow Discord from Claude Design's memory; their exact spacing is approximate.
- Icons, avatars, banners, nameplates and game art are placeholders; the port uses kyoko's icons and the gallery.
- The v1 tickets and `docs/parity.md` still say "matches oxidcord", and tickets 30, 31 and 37 landed on main in the oxidcord look. Switching their target is the orchestrator's and the user's call; raise it, don't edit them.
- Not started: the port itself. Suggested first step: Midnight's palette and sizes as kyoko theme tokens (`apps/desktop/src/theme.rs` and `ui/theme.luau`), then the floating-panel shell in `ui/shell/frame.luau`, which restyles every existing screen at once.

## Brief given to the second chat: expressions and commands

The user flagged this as the next important area: emoji, slash commands, GIFs, stickers and the rest of what people put into messages. What exists today and what needs designing:

- **Unicode emoji.** Kyoko draws them as text in the system emoji font (Segoe UI Emoji, `docs/ui.md`); Discord draws its own image set everywhere (messages, reactions, picker, composer, names). A 1:1 look needs an emoji image set, which raises licensing and bundle size (spec story 6 asks that the emoji table's origin and licence be established). Decide with the user.
- **Custom and animated emoji.** Rendered through the gallery (`timeline::Pictures`); animated ones play only on screen. Still to design: Nitro-locked emoji in the picker and autocomplete (Discord greys them and shows a lock), favourites and frequently used, super reactions.
- **Emoji picker.** A mockup exists (emoji tab). Ticket 32 (claimed on the other PC) builds the pickers to oxidcord; reactions already use a Rust picker (`src/timeline/picker.rs`).
- **Slash commands.** Ticket 27 runs them but has no option autocomplete or file options. Discord's flow needs pages: the command picker grouped by app with icons, option chips (required and optional), choices and autocomplete, attachment options, "is thinking...", ephemeral replies ("Only you can see this" with Dismiss), "The application did not respond", and message components on bot messages (buttons, selects, modals). Mockbot in mockcord has `/ping`, `/echo`, `/roll`, `/broken`.
- **GIFs.** Picker mockup exists (categories and trending); still to design: search results, favourites (the star on hover), what a sent GIF looks like, and the autoplay setting.
- **Stickers.** Picker mockup exists; sticker messages render (Lottie stickers draw nothing, as in oxidcord). Nitro limits to design.
- **The rest.** The composer's apps and gift buttons (decide whether kyoko shows them), poll creation, voice message playback, markdown highlighting inside the composer, the upload tray, link previews while typing. Voice and the soundboard are out of v1.

Method that worked: write the brief from the spec and the code, have Claude Design draft pages, download and review every page at 1280 by 800, send precise fixes, commit. Capture the live client read-only where it helps, with the user's go-ahead.

## Tools

- Claude Design project "Midnight Discord HTML recreation": `https://claude.ai/design/p/29248525-0762-464d-806c-def5d8ad3f73`, in the `metabrain` use-browser Chrome profile (`BU_SESSION=metabrain use-browser launch chrome`), which is logged in. Pick design system None. Its chat file input only exists while uploading: patch `HTMLInputElement.prototype.click` to keep the input, append it to the body, then `DOM.setFileInputFiles` by node id in a batch. Uploads wait until a running turn ends.
- Export: pages are served from the preview iframe's URL with `/serve/export/<Page Name>.html?<token>` (take the token from the iframe `src`). Strip the two injected `<script>` tags (`data-omelette-injected` and `__om_srcmap`) and save as lower-case kebab names.
- Review: render each page in the browser at 1280 by 800 (`Emulation.setDeviceMetricsOverride`); a tab in the background times out on screenshots, so bring it to the front. Four pages per contact sheet (iframes scaled 0.5) make a full pass quick.
- Live captures: `..\winuse\winuse.exe get-window-state <window> --screenshot` and `zoom` read the window without acting on it. Any click or key brings Discord to the front, so ask first, restore what you change, and stay out of it while the user streams.
