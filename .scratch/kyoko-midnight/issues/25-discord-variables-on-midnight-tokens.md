# 25 — Discord theme variables on the Midnight tokens

**What to build:** Map Discord's own CSS variables in `CSS_MAPPING` onto the Midnight tokens the restyled shell reads, so a Vencord or BetterDiscord theme that sets only Discord variables colours kyoko again. Suggested by M03: `background-primary` and `background-secondary` to `bg_4`, `background-tertiary` to `bg_3`, `text-normal` to `text_3`, `header-primary` to `text_2`, `interactive-normal`/`-hover`/`-active` to `text_4`/`text_3`/`text_1`, `background-modifier-hover`/`-selected` to `hover`/`active`. A theme setting only one surface then moves what derives from it; values a theme sets itself still win.

**Blocked by:** 01, 03; v1 57

**Status:** resolved

**Spec:** `docs/design/midnight.md` (intro: kyoko reads a theme's variable block), `docs/themes.md`

- [x] A Discord-variable theme recolours the Midnight shell (import test with a theme that sets only Discord variables)
- [x] No variable lands on two tokens unless documented, and the existing import tests still hold
- [x] `docs/themes.md` lists the mapping

## Answer

Merged to main as `dd76652` (rebased by the orchestrator onto M11, recorded under its rebased id on main). Docs: `docs/themes.md` (mapping table and rules), `docs/parity.md` (`KYOKO_CAPTURE_THEMES`).

- `CSS_MAPPING` (`crates/bridge/src/themes/css.rs`): the Midnight token rows gained Discord names (first found wins): `bg_4` ← `--background-primary`, `--background-base-low`, `--background-secondary`, `--background-base-lower`, `--background-secondary-alt`; `bg_3` ← `--background-tertiary`, `--background-base-lowest`; `bg_2`/`bg_1` ← `--button-secondary-background`/`-hover`; `hover`/`active`/`active_2` ← `--background-modifier-hover`/`-selected`/`-active`; `message_hover` ← `--background-message-hover` (its two rows merged into one); `border` ← `--background-modifier-accent`; `text_0` ← `--white-500`; `text_1` ← `--interactive-active`; `text_2` ← `--header-primary`, `--text-strong`; `text_3` ← `--text-normal`, `--text-default`, `--interactive-hover`; `text_4` ← `--interactive-normal`, `--channels-default`, `--text-muted`, `--header-secondary`, `--text-subtle`; `text_5` ← `--interactive-muted`; `accent_1` ← `--text-link`; `accent_2` **and** `accent_3` ← `--brand-experiment`, `--brand-500`, `--brand-experiment-500` (the one documented two-token case); `accent_4` ← `--brand-experiment-560`, `--brand-560`; `online`/`idle`/`dnd`/`offline` ← `--status-*`.
- Discord names still also land on oxidcord's tokens (screens still read them), each built on the same Midnight token. On a Midnight token its own variable (`--bg-4`) now wins over the Discord names (before, Discord names won). Panels lose the primary/secondary split: both feed `bg_4`, primary wins.
- Left unmapped on purpose: `--background-floating`, `--focus-primary`, `--mention-background` (oxidcord tokens only); Discord's danger/warning/positive colours don't reach Midnight's red/yellow/green scales (if a restyled screen should follow a theme's danger colour, map `--status-danger`/`--text-danger` onto `red_2` and decide `dnd`).
- Capture hook `KYOKO_CAPTURE_THEMES=<file>,<file>` copies theme files into the capture's themes folder and turns them on.
- Tests (bridge): own fixture `fixtures/discord-variables.theme.css` (dark, light and an ignored sidebar-scoped block); `a_theme_of_discord_variables_recolours_the_midnight_shell`, `a_discord_theme_recolours_a_light_base_from_its_light_block_only`, `a_discord_variable_lands_on_one_midnight_token_and_what_is_built_on_it`. Changed assertion in `a_vencord_theme_brings_only_its_colour_variables`: on a light base `user_panel_background` now follows `bg_4` (the derivation the ticket asks for).
- Checks re-run by the orchestrator in lane C (full list on `5185731`, then fmt, clippy, build, `xtask test` and captures again after rebasing onto M11): all green. Mockup parity (port 8493): timeline 17.5%, settings-appearance 11.6%, server-settings-roles 4.3%, command-picker 11.5% (all unchanged); with the fixture theme on, timeline 44.0% (whole palette changed, as expected).
- **Oxidcord token cleanup (M01) still blocked.** Still reading oxidcord colour tokens: Luau `components/avatar`, `mention_badge`, `status_dot`; `settings/keybinds`, `plugins`, `widgets`; `shell/command_picker`, `composer`, `dm_sidebar`, `friends`, `plugin_notice`, `storage_notice`; about 13 Rust files read `link` and as many `danger`. When the last goes, remove the oxidcord rows; the mapping test then reduces to one Midnight token per Discord variable.
