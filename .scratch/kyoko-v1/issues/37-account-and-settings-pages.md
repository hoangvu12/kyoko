# 37 — Account and settings pages

**What to build:** The account and settings pages match oxidcord and include kyoko's built-in toggles:

- image quality, including original resolution (the built-in FixImagesQuality)
- animation
- reduced motion
- typing animation (the built-in NoTypingAnimation)
- link warning (the built-in AlwaysTrust)
- the safety setting

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** claimed

- [ ] Settings and account pages match oxidcord in the parity captures
- [ ] Each built-in toggle takes effect
- [ ] The voice, updater and WASM-plugin pages are left out

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).
