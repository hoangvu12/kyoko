# 19 — Settings as Discord's dialog

**What to build:** Settings become a rounded dialog over the dimmed app (52 in from each edge at 1280 by 800, window controls above the dim) with Discord's sidebar: profile card, search, the groups (kyoko's plugin and theme pages in Vencord's place), the selected item's sub-sections, Log Out, build info; pages in the centred column with section headings, label and description left, control right, dividers and Related Settings cards. Add the expression settings Discord has (GIF autoplay, animated emoji, sticker animation) where the client puts them.

**Blocked by:** 01, 07; v1 37, 45, 57

**Status:** claimed

**Mockups:** `docs/design/mockups/settings-my-account.html`, `docs/design/mockups/settings-notifications.html`, `docs/design/mockups/settings-appearance.html`, `docs/design/mockups/settings-plugins.html`, `docs/design/mockups/settings-themes.html`

**Spec:** `docs/design/midnight.md`, Settings

- [ ] Matches the mockups
- [ ] Every existing settings page is reachable from the new sidebar
- [ ] Search filters pages and settings

## Design brief (confirmed by the user, 2026-10-26, through `/impeccable shape`)

1. **Feature summary.** Settings become Discord's current settings dialog over the dimmed app, in Midnight. For someone who opens settings to change one thing, finds it in seconds by sidebar or search, and gets back to chatting. Every existing kyoko settings page stays reachable.
2. **Primary action.** Find a setting and change it; search or a sidebar click goes straight to it.
3. **Direction.** Midnight as specified (Restrained: warm neutrals, `accent_2` only for selected state, switches and focus). Scene: a long-time Discord user in the evening, in a dim room, popping into settings mid-conversation. Anchors: Discord's 2025 settings dialog (layout), refact0r's Midnight (surfaces), Vencord's settings placement (kyoko's group).
4. **Scope.** Production-ready, the whole settings surface, checked against `settings-my-account`, `-notifications`, `-appearance`, `-plugins`, `-themes` with `cargo xtask parity mockup`. Server settings stay separate (M23).
5. **Layout.** A `kit::dialog` inset `settings_dialog_inset` 52 over M07's scrim, window controls bright above the dim, close X top right. Sidebar: profile card (avatar, name, "Edit Profiles"), search, groups, Log Out in red, build info at the bottom; hovered `bg_3`, selected a `bg_2` pill sliding 10 on hover, the selected page's sub-sections under it with a thin line on the current one. Content: page title in the top bar, a centred column of about 696, large section headings, label and description left, control right, thin dividers, "Related Settings" link cards. Groups (today's pages mapped):
   - **Account:** My Account (Profiles as its sub-section / "Edit Profiles"), Data & Privacy (today's Privacy & Safety, plus **Activity Privacy**: share activity status), Notifications, and today's note pages **kept** (user decision): Devices, Connections, Friend Requests, Profiles, each pointing to the web app as today.
   - **Kyoko** (between Account and where Billing would be, Vencord's place): Plugins, Themes, and kyoko's own safety, storage and privacy settings as they live today.
   - **Experience:** **Voice & Video, greyed** (listed dimmed, opens a short page "Kyoko has no voice calls"; nothing changeable), Appearance, Accessibility, Chat, Keybinds, Language & Time, Streamer Mode.
   - **Developer:** today's Advanced (developer mode, credits).
   - **Log Out.**
   - **Left out:** Billing, Nitro, Clips (kyoko's current Clips note page may go: it says "not supported", not "managed in the web app"; the implementer decides), Registered Games, Game Overlay.
6. **States.** Open: M07's modal motion (none under reduced motion), last page visited (default My Account), search focused. Search: pages and settings; a query narrows the sidebar to pages with matches and highlights matching rows on the open page; no match shows "No settings match '<query>'"; clearing restores the sidebar on the same page. Greyed Voice & Video as above. Plugin settings pages (generated from manifests) under Plugins, suspended plugins show their notice. Settings apply at once as today; settings-proto rollbacks keep their toast. Narrow windows: the inset shrinks before the content column; the sidebar keeps its width.
7. **Interaction.** Opened from the user panel's gear (and Ctrl+, if bound today); Escape, the X or an outside press closes it and focus returns. Up/Down through the sidebar, Enter opens a page, Tab through controls; search filters as you type; a sub-section click scrolls to its heading. Log Out uses the existing sign-out path.
8. **Content.** Group and page titles as above (Discord's wording where a page matches Discord's), the no-match line, the Voice & Video line, build info (version, build, data folder link). Every setting needs searchable text (label, description, extra keywords); pages hold a handful up to ~20 settings (Appearance).
9. **References.** `docs/design/midnight.md` ("Settings", "Other surfaces"); M07's `kit::switch`/`radio_row`/`checkbox`; impeccable's `spatial-design.md`, `interaction-design.md` (keyboard, focus return), `motion-design.md` (reduced motion).
10. **Open for the implementer.** Place GIF autoplay, animated emoji and sticker animation where `settings-appearance.html` and Discord put them (Accessibility or Chat). Place Keybinds and Streamer Mode as the mockup's sidebar does (its tabs: account, privacy, messaging, notifications, clips, vencord, plugins, themes, updater, cloud, backup, nitro, boost, subs, gifts, billing, voice, appearance, accessibility, system, language, games, activity, overlay, apps, developer, logout). M20 notes: Edit Profile opens the settings Profiles page via `Request::EditProfile`; Log Out now lives only in settings, keep it there; `profile/parts.rs` has reusable parts and a Profiles preview could reuse `popout::card`.
