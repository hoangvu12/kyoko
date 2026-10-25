# 02 — Compare captures with the mockups

**What to build:** The parity tooling (v1 19) compares kyoko with oxidcord. Add a mode that compares a kyoko capture with a mockup from `docs/design/mockups/`: render the mockup at 1280 by 800 (for example with a locally installed Chrome or Edge in headless mode; the mockups load Inter from Google Fonts and Twemoji from jsDelivr), capture kyoko at the same size against mockcord, and write the pair and the difference as the oxidcord mode does. Map each capture screen to its mockup page.

**Blocked by:** v1 19 (parity capture tooling)

**Status:** claimed

**Mockups:** every page

**Spec:** `docs/design/midnight.md`, Mockups

- [ ] `cargo xtask` can capture a screen and its mockup side by side at 1280 by 800
- [ ] The screen-to-mockup mapping is documented in `docs/parity.md`
- [ ] Fails with a clear message when no browser is installed
