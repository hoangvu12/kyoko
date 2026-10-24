# 02 — Strip GPUI UI, voice, WASM plugins and updater

**What to build:** The workspace holds only what kyoko keeps. The session runs on its own: no voice, no WebAssembly plugin host, no updater, and no way for a plugin failure to stop startup. This clears the way for zgui, whose crate patches would conflict with GPUI.

**Blocked by:** 01 (Rebrand fork to kyoko)

**Status:** ready-for-agent

- [ ] The GPUI ui crate, voice, plugin-host, plugin-sdk, updater, the WASM first-party plugins and the plugin packs are removed
- [ ] The session builds and runs without voice or plugin dependencies. An optional hook interface exists for a future plugin runtime, and a hook failure cannot stop the session
- [ ] Code paths and packaging used only on Linux or macOS are removed or left uncompiled
- [ ] The headless mock run passes, minus its voice and plugin steps
- [ ] The workspace builds on Windows with fmt and clippy clean
