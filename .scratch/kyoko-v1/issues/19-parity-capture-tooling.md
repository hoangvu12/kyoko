# 19 — Parity capture tooling

**What to build:** A contributor can capture every parity screen from oxidcord and kyoko against the same mockcord seed, and compare them side by side.

**Blocked by:** 04 (Spike B: live messages, WebP avatar and findings)

**Status:** resolved

- [x] A pinned oxidcord reference build (0.2.0-beta.1) runs from a separate checkout outside kyoko's tree
- [x] Both apps can be driven to a named screen and state on the same mockcord seed
- [x] One command captures both sets of screenshots for every screen in the parity list
- [x] The output pairs the two captures for each screen, for review

## Answer

Merged to main as `ebd4778..c02fd5f`. Docs: `docs/parity.md` (the screen list xtask reads), `tools/parity/README.md`.

- Reference: `tools/parity/oxidcord-ref.ps1` clones oxidcord into `oxidcord-ref` beside the kyoko checkout, checks out `b852253`, applies `tools/parity/oxidcord-ref.patch` (about 650 lines: `--mock-server <url>`, `--capture-screen`, `--capture-ready`, a capture driver `crates/ui/src/shell/capture.rs`, small accessors/visibility changes, `anyhow` in its ui crate) and builds (debug; `-Release`, `-Force`, `-Path`, `-Export` to regenerate the patch). The reference build is 6.7 GB. `tools/parity` is excluded from the workspace.
- One mockcord: `mockcord --seed 1 --port 8485 --anchor 2026-10-01T12:00:00Z`, restarted between apps; both apps get `--mock-server http://127.0.0.1:8485/` and a fresh data dir (kyoko v9, oxidcord v10, token `mock-token`). Same guild, channels and messages on both sides.
- Command: `cargo xtask parity capture [--screens a,b] [--apps oxidcord|kyoko] [--oxidcord exe] [--seed] [--anchor] [--port] [--out] [--no-build]`; `cargo xtask parity list`. Per app: wait for the ready file, find the largest visible window of the process (EnumWindows by PID), `PrintWindow` the client area (screen-copy fallback), `WM_CLOSE` then kill after 3 s. Output `target/parity/<screen>/{oxidcord,kyoko,pair,diff}.png`, logs, and `target/parity/index.html` with differing-pixel share. 1280×800 at 100% scale, dark theme. Non-zero exit if any capture failed.
- Parity list: 73 screens grouped by tickets 20–38 (app-frame; login-tos, login-token; server-rail(-folder); channel-sidebar(-collapsed); dm-sidebar, dm-group; timeline; timeline-media; composer(-reply); message-hover, message-context-menu; member-list; user-panel, status-menu, custom-status, profile-popout, full-profile; quick-switcher, search; lightbox, emoji/gif/sticker pickers; threads/forums/pins; friends-*; add-server, forward, guild-notifications; server-settings-* and moderation-*; settings-* (17 pages); toast, connection-offline). A full run took 26.6 min under load; on a quiet machine about 7 s per oxidcord screen.
- kyoko capture mode: hidden `--mock-server`, `--capture-screen`, `--capture-ready` flags in `apps/desktop`; `apps/desktop/src/capture.rs` (`Screen::named`); `Link::settled` in `ui.rs` (launch channel open, messages loaded, avatar shown via the gallery); waits use `zgx::timer::sleep`; a screen without steps exits with code 3 and gets a placeholder. Today the spike window differs from oxidcord by about 11% on app-frame and timeline.
- How a UI ticket adds kyoko's capture: add the name in `Screen::named` (new variant if it needs steps), drive it through real commands/actions, extend `Link::settled`, run `cargo xtask parity capture --screens <name>` and compare `pair.png`. A new screen also needs a row in `docs/parity.md` and oxidcord steps in the reference checkout's `capture.rs`, then `oxidcord-ref.ps1 -Export` and commit the patch.
- Quirks: "Today/Yesterday" wording depends on the wall clock and kyoko shows UTC until 25; ticket 39 needs a locale option in capture mode; oxidcord's single-instance guard means instances are killed between captures; its title can be "(N) Oxidcord", so windows are matched by PID.
