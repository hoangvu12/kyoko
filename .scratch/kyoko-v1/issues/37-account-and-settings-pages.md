# 37 — Account and settings pages

**What to build:** The account and settings pages match oxidcord and include kyoko's built-in toggles:

- image quality, including original resolution (the built-in FixImagesQuality)
- animation
- reduced motion
- typing animation (the built-in NoTypingAnimation)
- link warning (the built-in AlwaysTrust)
- the safety setting

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] Settings and account pages match oxidcord in the parity captures (except Profiles, see the answer)
- [x] Each built-in toggle takes effect
- [x] The voice, updater and WASM-plugin pages are left out

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `5dad972..dcf555a`. Docs: Settings in `docs/ui.md` (with a page-registration example), `docs/parity.md`, `docs/desktop.md`, `docs/locks.md`.

- Bridge: typed settings service `crates/bridge/src/settings.rs` (`SettingKey`, saves `settings.json`, lenient load); `LinkAction::warned` in bridge decides whether an untrusted host confirms (`timeline/links.rs` uses it). Desktop `apps/desktop/src/ui/settings.rs` applies each change live where it can: theme via `Catalog::apply`, gallery playback/reduced motion/original resolution, link warning and trusted hosts, notifications, minimize to tray, autostart and link handler, and the show/hide hotkey (now re-registered live). Safety locks are read at the next session start.
- Luau: `ui/shell/settings.luau` (opened from the user panel cog; the timeline slot is collapsed while open and restored on close unless a home page covers it), pages in `ui/settings/*.luau`, shared widgets in `ui/settings/widgets.luau`, registry `ui/settings/pages.luau`. Log Out calls `login.sign_out`. Voice & Video, Updates and oxidcord's own section are left out; developer mode lives on Advanced (no Developer page).
- Built-in toggles shown to take effect: original resolution (`the_original_resolution_setting_fetches_the_original_attachment`), animation and reduced motion (gallery test), link warning (`the_link_warning_setting_decides_whether_unknown_hosts_confirm`), safety locks (`the_safety_locks_setting_is_read_at_the_next_session_start`), typing animation (`the_typing_dots_animate_only_while_the_setting_allows`, headless via 60's `TestApp`, dots `id = "typing_dot_<n>"` in `composer.luau`). All 15 pages are in 60's load check (`SETTINGS_PAGES` in `apps/desktop/src/ui/scripts.rs`).
- Saved but without effect yet (listed in `docs/ui.md`): compact messages, DPI and font scaling, saturation, 24-hour time, emoji autocomplete, hiding invites, GIF usage.
- Parity (port 8496, captured at 150% display scale, 1920×1200 on both sides): my-account 3.0%, profiles 10.9%, privacy 2.6%, devices 2.1%, connections 1.6%, clips 1.6%, friend-requests 2.5%, appearance 2.3%, accessibility 3.9%, chat 2.8%, notifications 2.4%, keybinds 2.8%, language 2.1%, streamer-mode 2.2%, advanced 4.4%. Not captured: `settings-themes` (57), `settings-developer` (no page). Verified by the orchestrator in lane B.
- Deviation accepted: Profiles is a note with an "Open Discord" button (as Devices and Friend Requests); oxidcord has a profile editor. The spec doesn't ask for profile editing; kept as a possible follow-up, not a ticket yet.
- For 45: register with `pages.add_section` / `pages.add_page` from `@settings/pages` before `shell/frame.luau` builds the shell (the nav is built once); a page is `{ key, title, icon, build }` (the `plugins` icon exists; keys use lowercase letters, digits, `-`, `.`, `_`); `widgets.toggle_row`/`toggle`/`setting_radios` only work with `bridge::SettingKey` keys, so plugin pages publish their own `plugin.<id>.*` values; map the capture screen in `settings_page` in `src/capture.rs`; add the page to `SETTINGS_PAGES`.
- For 57: add a Themes page through the same registry; Appearance already writes the `theme` key (Dark, Light, AMOLED, System); theme names allow letters, digits, `-`, `_`, `.` and spaces up to 64 chars; add the `settings-themes` capture mapping.
