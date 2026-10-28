# 85 — Hover states: one rule for text colour

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Many items turn their text dark on hover where Discord turns it white (or brighter). The user asks whether kyoko has a design system. It does (Midnight tokens, `docs/design/midnight.md`), so this is an audit: find every hover and selected state and make it follow one rule from the Midnight spec.

**Blocked by:** none

**Status:** resolved

- [x] Inventory of hover and selected states (sidebar rows, menus, buttons, tabs, picker items, settings rows) with their current text colour
- [x] One documented rule in `docs/ui.md` (hover and selected text colours), using Midnight tokens only
- [x] Every listed state follows it; captures of the main screens checked by eye

## Answer

Merged to main as `5c3655c` (worker `72a98d1d`, lane A; full verify re-run by the orchestrator: all green; `parity mockup` server-rail 17.5%, channel-sidebar 17.5%, search 14.9%, full-profile 28.6%, emoji-picker 20.7%, all unchanged from before since static captures barely show hover). Docs: `docs/ui.md` ("Hover and selected text", with the inventory table).

- Root cause: Midnight's `text_0` is dark (`#282828`) and meant only for text on coloured fills, but some hover states used it: rail tiles without an icon (accent fill, dark letters), Add a Server and Discover (`bg_4`), the selected Home tile, the current search page on hover, and the older settings button (`text_on_brand` for secondary and ghost buttons). Selected items also dimmed on hover (selected channel icon, full-profile tabs, emoji picker tabs `text_2` -> `text_3`), search tabs lost their selected background, Reset Nickname went `accent_1` -> `accent_2`.
- The rule: hover never darkens text; muted text goes up one step; selected text is `text_2` on `active` and stays put on hover (the settings sidebar keeps the mockup's `text_3`); `text_0` only on a coloured fill, where hover only darkens the fill; rail tiles are `text_1` on `active` when hovered or selected.
- Fixed in `server_rail.luau`, `channel_sidebar.luau`, `search_panel.luau`, `settings/widgets.luau`, `profile/full.rs`, `pickers/mod.rs`, `timeline/reactions.rs`, `moderation/dialogs.rs`. Test `ui::scripts::hovered_and_selected_text_never_turns_darker` (fails on the old Luau with the user's bug).
- Deviations: rail tiles no longer take an accent fill on hover or selection (the pill marks it, as in the main-screen mockup; departs from the spec's add-server "`--bg-4` hovered"); channel names brighten on hover with their icons; muted channels lift `text_5` -> `text_4`; Reset Nickname stays `accent_1` (zgx can't underline a plain button).
- For later: a zgx Luau `hover = {}` table takes fixed values only, so for selectable items track hover with `on_hover` and pick colours in a function. In Rust, `.hover(text_color)` beats a selected colour on the same node: put the hover colour on the parent row and the selected colour on the label.
