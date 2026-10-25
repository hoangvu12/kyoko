# 02 — Compare captures with the mockups

**What to build:** The parity tooling (v1 19) compares kyoko with oxidcord. Add a mode that compares a kyoko capture with a mockup from `docs/design/mockups/`: render the mockup at 1280 by 800 (for example with a locally installed Chrome or Edge in headless mode; the mockups load Inter from Google Fonts and Twemoji from jsDelivr), capture kyoko at the same size against mockcord, and write the pair and the difference as the oxidcord mode does. Map each capture screen to its mockup page.

**Blocked by:** v1 19 (parity capture tooling)

**Status:** resolved

**Mockups:** every page

**Spec:** `docs/design/midnight.md`, Mockups

- [x] `cargo xtask` can capture a screen and its mockup side by side at 1280 by 800
- [x] The screen-to-mockup mapping is documented in `docs/parity.md`
- [x] Fails with a clear message when no browser is installed

## Answer

Merged to main as `55e649c`. Docs: `docs/parity.md`, "Comparing with the Midnight mockups".

- Usage: `cargo xtask parity mockup --port <lane port> --screens <screen>[,<screen>...]`, then `target\parity-mockup\<screen>\pair.png` (mockup left, kyoko right), `diff.png`, or `target\parity-mockup\index.html`. No `--screens` runs all 58 mapped screens (4–5 min); `--no-build` reuses the build, `--browser <exe>` picks a browser. `cargo xtask parity list` shows each screen's mockup page.
- The mockup file is opened unchanged in a headless Chrome or Edge through DevTools (synchronous `tokio-tungstenite`, already a workspace dependency, now in xtask only). The screenshot waits for load, fonts, every image decoded and two frames; a stylesheet, image or Inter that failed marks that screen's mockup side failed, with the URLs in `<screen>/mockup.log`. Each run uses a throwaway profile and fetches Inter and Twemoji again (network needed). The browser is shut down at the end.
- Browser search: `--browser`, `KYOKO_PARITY_BROWSER`, Chrome or Edge under Program Files, Program Files (x86) and local app data, then App Paths. None found: `no Chrome or Edge found; install one, or pass --browser <path to chrome.exe or msedge.exe>, or set KYOKO_PARITY_BROWSER`, checked before building.
- Scale: the mockup renders at kyoko's window scale (kyoko capture width / 1280), so at 150% both sides are 1920×1200 and line up pixel for pixel; it warns only if kyoko's window isn't 1280×800 logical or the sizes still differ (then no `diff.png`).
- Mapping: `MOCKUPS` in `tools/xtask/src/parity/mockup.rs`, mirrored in `docs/parity.md`, with the kyoko screens that have no mockup and the 51 mockup pages with no kyoko screen yet. Seven screens share `midnight-discord.html`; `composer` and `composer-reply` share `composer-markdown.html`: compare the part the ticket is about. A screen without a mapping stops with "has no mockup yet". To map a new screen: kyoko capture steps, a row in the Screens tables, a `MOCKUPS` entry, a row in the mockup table, and remove it from the unmapped list.
- Baseline before the restyle (port 8496, Chrome, 150%): main-screen screens 19.0%, login-tos 12.4%, login-token 15.8%, dm-sidebar 14.8%, dm-group 9.7%, pickers 31–35%, full-profile 52.3%, lightbox 55.4%, forum-post-create 46.4%, friends tabs 8.6–11.3%, server settings 19.6–24.7%, settings-my-account 29.6%. Stand-in text, avatars and icons mean the share never reaches 0; judge layout, sizes, spacing and colour by eye. Checks and a two-screen capture re-run by the orchestrator in lane B.
- The oxidcord mode is unchanged apart from a shared refactor (`parity/{browser,compose,report,window}.rs`).
