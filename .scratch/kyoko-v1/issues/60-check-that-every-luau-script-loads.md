# 60 — Check that every Luau script loads

**What to build:** The standard checks catch a Luau UI script that no longer parses or fails to load, so a bad rebase or merge can't pass review unnoticed.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** claimed

- [ ] `cargo xtask test` (or `cargo test --workspace`) fails when any script under `apps/desktop/ui/` has a syntax error
- [ ] It also fails when the UI's entry module fails to load: a missing `require`, or a runtime error while the shell is being built
- [ ] It runs headless, with no window and no network, in a few seconds
- [ ] A deliberately broken script is shown to fail the check, then restored

## Comments

Found during 29: after a rebase, a merge cut an `end` out of `ui/actions.luau`. Build, clippy, every test, `xtask test` and the headless run all still passed; only a parity capture showed the UI failing to load.
