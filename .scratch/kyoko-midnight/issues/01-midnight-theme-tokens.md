# 01 — Midnight theme tokens and Inter

**What to build:** Midnight's palette and Discord's sizes become kyoko's built-in theme: every colour and size the mockups use exists as a token in `apps/desktop/src/theme.rs` and `ui/theme.luau`, named after Midnight's variables (`--bg-1`..`--bg-4`, `--text-0`..`--text-5`, `--accent-1`..`--accent-5`, `--hover`, `--active`, status colours, base scales, radii, spaces, the measured sizes). Inter (SIL OFL) is bundled as the interface font with Midnight's fallback list; `gg mono`, Source Code Pro, Consolas for code. oxidcord's tokens stay available until every screen is restyled.

**Blocked by:** none (coordinate with v1 57, which reads a Vencord/BetterDiscord variable block into these tokens)

**Status:** resolved

**Mockups:** `docs/design/mockups/midnight-discord.html (the `:root` block of any page holds every token)`

**Spec:** `docs/design/midnight.md`, Palette, Type, Sizes Discord sets and Midnight keeps

- [x] Every `:root` variable of the mockups has a kyoko token, or a documented reason it is not needed
- [x] A Midnight theme is selectable and is the default; switching themes restyles without restarting
- [x] Inter is bundled with its licence in `THIRD_PARTY_NOTICES.md` and used when the theme asks for it
- [x] `docs/ui.md` describes the new tokens

## Answer

Merged to main as `be43d34`. Docs: `docs/ui.md` ("Midnight tokens", Fonts), `docs/themes.md`, `THIRD_PARTY_NOTICES.md`.

- Tokens in `crates/bridge/src/themes/tokens.rs` and `ui/theme.luau`, named after Midnight's variables (`--bg-4` is `bg_4`, `--chatbar-height` is `chatbar_height`): 72 colour and 199 size tokens; Luau and Rust read the same zgx tokens `kyoko.<name>` (Rust also `Palette::size`/`Palette::color`; where a Luau property takes only a number, `theme.number(theme.size.gap)`). Opacities are size tokens (e.g. `disabled_opacity`).
- Derivations (`DERIVED_COLORS`, derived sizes following Midnight's `calc()`): changing `purple_2` changes `accent_2`, `mention`, `brand`, `focus_ring`; rail = icon + 2×12 + 2×1; header = icon + 8 + 2; oxidcord's tokens take Midnight values (`background_primary` from `bg_4`, `text_normal` from `text_3`). A value a theme sets itself wins.
- Midnight (refact0r, Vencord flavour 2.1.1, values from the real flavour file, MIT licence in the notices) is built in, selectable (Themes page, Appearance) and the default. Saved settings are read as they are (no migration, per the user). The old oxidcord `themes/midnight` is now `themes/deep-blue` ("Deep Blue").
- CSS import: `CSS_MAPPING` is one-to-one for Midnight (each variable one token); the Discord column is unchanged and still lands only on oxidcord's tokens. Import now reads `color-mix()` and plain-pixel sizes (`CSS_SIZES`). `MOCKUP_ROLES` maps Discord semantic names used in the mockups to Midnight tokens; `UNTOKENED` lists the rest with reasons (gradients, filters, relative emoji sizes, font weights, stand-in art, window size).
- Inter 4.1, static 400/500/600/700, compiled in and registered with `Application::font` at start; release zip and per-file SHA-256 in the notices (`*.ttf binary` in `.gitattributes`). Only Midnight asks for Inter; Dark, Light and AMOLED keep oxidcord's font lists.
- Radii follow Discord's scale for every screen: `radius_xs` 4 (new), `radius_sm` 8 (was 4), `radius_md` 12 (was 8). Seven Midnight sizes have their own names because oxidcord uses the name with another value: `member_list_panel_width`, `channel_item_height`, `message_content_left`, `modal_width_wide`, `settings_dialog_nav_width`, `profile_dialog_banner_height`, `top_strip_height` (Midnight's `--top-bar-height`).
- Tests (bridge): `every_variable_of_the_mockups_has_a_token_or_a_reason` (all 369 `:root` variables of `midnight-discord.html`), the real flavour and the mockup `:root` both import to exactly the built-in palette on Dark and Light bases, every token from Midnight on any base, no Midnight variable on two tokens, derivations follow a changed base, Midnight is the default and credits refact0r. The load check builds every screen under Midnight. Checks re-run by the orchestrator in lane A (one session test and 48's headless step failed under load / known flake; both pass alone).
- Mockup parity (port 8494): main-screen screens 19.7% (19.0% before), settings-appearance 28.0%, settings-my-account 30.9%; the layout is still oxidcord's, but sampled pixels match the mockup (`#282828` panels on `#31302f`), text is warm, accents rose, font Inter.
- For M03: use only Midnight tokens. Window `bg_3`; panels `bg_4`, `radius_lg`, `border_thickness` border in `border`, spaced by `gap`, inset `gap` left, right and bottom. Sizes: `top_strip_height` 12, `guild_list_width` 66, `guildbar_avatar_size` 40, `guild_list_padding` 12, `sidebar_width` 280, `guild_sidebar_width` 346, `channel_header_height` 50, `chatbar_height` 47, `member_list_panel_width` 264, `channel_item_height` 36 / `channel_item_pitch` 38, `member_row_height` 42 / `member_row_pitch` 44, `list_item_hover_shift` 10, `member_hover_shift` 18, `window_control_size` 14, `window_controls_padding` 6. Use derived tokens rather than adding numbers in Luau. Look up Discord names in `MOCKUP_ROLES` before adding a token. Decide when the shell lands whether Discord variable names also map onto the new tokens (a Discord theme setting only primary would then recolour secondary via derivations). When the last oxidcord screen goes, remove its tokens and `DERIVED_COLORS` rows and give the seven renamed sizes their Midnight names.