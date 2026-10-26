# 25 — Discord theme variables on the Midnight tokens

**What to build:** Map Discord's own CSS variables in `CSS_MAPPING` onto the Midnight tokens the restyled shell reads, so a Vencord or BetterDiscord theme that sets only Discord variables colours kyoko again. Suggested by M03: `background-primary` and `background-secondary` to `bg_4`, `background-tertiary` to `bg_3`, `text-normal` to `text_3`, `header-primary` to `text_2`, `interactive-normal`/`-hover`/`-active` to `text_4`/`text_3`/`text_1`, `background-modifier-hover`/`-selected` to `hover`/`active`. A theme setting only one surface then moves what derives from it; values a theme sets itself still win.

**Blocked by:** 01, 03; v1 57

**Status:** claimed

**Spec:** `docs/design/midnight.md` (intro: kyoko reads a theme's variable block), `docs/themes.md`

- [ ] A Discord-variable theme recolours the Midnight shell (import test with a theme that sets only Discord variables)
- [ ] No variable lands on two tokens unless documented, and the existing import tests still hold
- [ ] `docs/themes.md` lists the mapping
