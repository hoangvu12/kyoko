# 02 — Strip GPUI UI, voice, WASM plugins and updater

**What to build:** The workspace holds only what kyoko keeps. The session runs on its own: no voice, no WebAssembly plugin host, no updater, and no way for a plugin failure to stop startup. This clears the way for zgui, whose crate patches would conflict with GPUI.

**Blocked by:** 01 (Rebrand fork to kyoko)

**Status:** resolved

- [x] The GPUI ui crate, voice, plugin-host, plugin-sdk, updater, the WASM first-party plugins and the plugin packs are removed
- [x] The session builds and runs without voice or plugin dependencies. An optional hook interface exists for a future plugin runtime, and a hook failure cannot stop the session
- [x] Code paths and packaging used only on Linux or macOS are removed or left uncompiled
- [x] The headless mock run passes, minus its voice and plugin steps
- [x] The workspace builds on Windows with fmt and clippy clean

## Answer

Merged to main as `a839e11..a694d8c` (four commits, about 85k lines removed). fmt, clippy `-D warnings`, `build --locked`, `cargo test --workspace`, `headless-run --mock` and `cargo deny check` all pass. CI now also runs `cargo test --workspace --locked`.

- Hook interface: `session::SessionHooks` (`crates/session/src/hooks.rs`), held as `SessionConfig::hooks: Option<Arc<dyn SessionHooks>>`. No-op defaults for `name`, `before_send`, `before_edit`, `allow_typing`, `allow_notification` (exposed as `Session::should_notify`, the session raises no notifications itself) and `on_dispatch` (called synchronously on the session task after each dispatch is applied). Every call is wrapped in `catch_unwind`; failures log only hook and owner names. No cancel-send; a hook can only rewrite.
- `Session::start` returns `io::Result` instead of panicking. Rendering is plain `markdown::parse`. The theme-catalog model lives in `session/src/catalog.rs`, still fail-closed with no key; `xtask theme keygen` exists for ticket 57.
- `apps/desktop` is a throwaway stub: `kyoko --mock` runs mockcord plus the session headless and logs counts. Not wired any more: `refresh_client_identity` (build-number scrape), the web-login helper entry point, single-instance forwarding and AUMID setup; recover them from `2f20b49:apps/desktop/src/main.rs` when needed.
- The workspace has no `[patch]` section, so 03 can add zgui's cleanly.
- `state::Changes` has a `theme_catalog` flag; the `plugins`/`plugin_slots` flags, `UserOverride` and `by_plugin` remain in `state` unused.
- For 05: `crates/session/tests/hooks.rs` has a `Running` helper (mockcord, scratch store, notify-based `until`) that seeds the harness. Tests and `--mock` turn on the plaintext-token setting so the mock token never reaches Credential Manager.
- For 45: `on_dispatch` runs on the session task, so a Luau runtime must queue rather than run scripts inline; a panicking hook still prints Rust's default panic message to stderr (install a quiet panic hook).
- `cargo deny` warns about a yanked `clipboard-win` (via `platform`) and two unused licence entries in `deny.toml`.
