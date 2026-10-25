# 60 — Check that every Luau script loads

**What to build:** The standard checks catch a Luau UI script that no longer parses or fails to load, so a bad rebase or merge can't pass review unnoticed.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] `cargo xtask test` (or `cargo test --workspace`) fails when any script under `apps/desktop/ui/` has a syntax error
- [x] It also fails when the UI's entry module fails to load: a missing `require`, or a runtime error while the shell is being built
- [x] It runs headless, with no window and no network, in a few seconds
- [x] A deliberately broken script is shown to fail the check, then restored

## Comments

Found during 29: after a rebase, a merge cut an `end` out of `ui/actions.luau`. Build, clippy, every test, `xtask test` and the headless run all still passed; only a parity capture showed the UI failing to load.

## Answer

Merged to main as `46a6ca5..f696def`. Docs: "The load check" in `docs/ui.md`.

- Two tests in `apps/desktop/src/ui/scripts.rs` (`#[cfg(test)]` module of the `kyoko` binary), also run by `cargo xtask test` (`cargo test -p kyoko --bin kyoko ui::scripts`) and by `cargo test --workspace`. Run alone with that command; 0.1–0.5 s once built, no window, no network.
- `every_script_compiles` parses every `.luau` under `apps/desktop/ui/` and lists each failing file with its line.
- `the_interface_loads_and_builds_every_screen` loads `app.luau` in zgx-script's windowless `testing::TestApp` with the real theme, registers a stub action and function for every `"area.name"` string literal the Rust host uses, sets sample shared values and builds five screens (ToS notice, token form, DMs, a server with channels and members, home page). Each screen must show a text only it has, so a screen that silently drops out fails too.
- Shown failing, then restored: a cut `end` in `ui/actions.luau`, a bad `require` in `shell/frame.luau`, a runtime error in `shell/member_list.luau` (each names file and line). Re-checked by the orchestrator.
- Only new dependency: `mlua` 0.12.2 (`luau`) as a dev-dependency, already patched to zgx's copy, so `Cargo.lock` gains no crate. `luau0-src` still `0.22.0+luau740`.
- For later UI tickets: host names must be registered as string literals (directly or in a literal list looped over); a name built with `format!` gets no stub and fails the check. A new screen or region adds its sample shared values and a unique text to `shows(...)` in `scripts.rs`. The "launch the app after every rebase" workaround is no longer needed.
