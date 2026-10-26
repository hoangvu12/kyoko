# 01 — Midnight theme tokens and Inter

**What to build:** Midnight's palette and Discord's sizes become kyoko's built-in theme: every colour and size the mockups use exists as a token in `apps/desktop/src/theme.rs` and `ui/theme.luau`, named after Midnight's variables (`--bg-1`..`--bg-4`, `--text-0`..`--text-5`, `--accent-1`..`--accent-5`, `--hover`, `--active`, status colours, base scales, radii, spaces, the measured sizes). Inter (SIL OFL) is bundled as the interface font with Midnight's fallback list; `gg mono`, Source Code Pro, Consolas for code. oxidcord's tokens stay available until every screen is restyled.

**Blocked by:** none (coordinate with v1 57, which reads a Vencord/BetterDiscord variable block into these tokens)

**Status:** claimed

**Mockups:** `docs/design/mockups/midnight-discord.html (the `:root` block of any page holds every token)`

**Spec:** `docs/design/midnight.md`, Palette, Type, Sizes Discord sets and Midnight keeps

- [ ] Every `:root` variable of the mockups has a kyoko token, or a documented reason it is not needed
- [ ] A Midnight theme is selectable and is the default; switching themes restyles without restarting
- [ ] Inter is bundled with its licence in `THIRD_PARTY_NOTICES.md` and used when the theme asks for it
- [ ] `docs/ui.md` describes the new tokens
