# Contributing

Read `AGENTS.md` first. It holds the rules every change follows.

## Setup

Rust 1.99 is pinned by `rust-toolchain.toml`. Kyoko v1 builds on Windows only and needs the MSVC toolchain, CMake and libclang.

```bash
cargo run -p kyoko -- --mock
```

### BoringSSL (`crates/net`)

`crates/net` uses `wreq`, which builds BoringSSL from source through `btls-sys`. That build needs:

- **CMake 3.22 or newer.** If `cmake` is not on `PATH`, point the `CMAKE` environment variable at it, for example the copy inside Visual Studio Build Tools (`...\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe`), and set `CMAKE_GENERATOR` to `Visual Studio 17 2022`. Building from a "Developer PowerShell for VS 2022" also works.
- **libclang**, for bindgen. The official LLVM installer puts it in `C:\Program Files\LLVM\bin`, where it is found without any setting. Anywhere else, set `LIBCLANG_PATH` to the folder holding `libclang.dll`.
- **Git**, which `btls-sys` runs to apply its BoringSSL patches.

NASM and Perl are not needed. `btls-sys` builds BoringSSL's assembly unless it is cross-compiling, so a native Windows build would ask for NASM. `.cargo/config.toml` sets `HOST_CMAKE_TOOLCHAIN_FILE` to `.cargo/boringssl-no-asm.cmake`, which turns on `OPENSSL_NO_ASM` for that build only; nothing else in the tree builds through CMake. If a BoringSSL build ever failed before this setting was in place, delete `target/<profile>/build/btls-sys-*/out/build`, because CMake keeps the first configuration.

The first build of BoringSSL takes several minutes per target directory and profile (dev and release each build it once); later builds reuse it. sccache does not cache it. The first `cargo fetch` clones BoringSSL into `~/.cargo/git` (about 650 MB). Changing any of the `BORING_BSSL_*`, `CMAKE*` or `HOST_CMAKE_TOOLCHAIN_FILE` variables rebuilds it, so keep them the same between builds. Measurements are in `docs/fingerprint.md`, "Build cost".

## Before opening a pull request

```bash
cargo xtask check-all
cargo xtask headless-run --mock
```

## Pull requests

Keep them small and on one topic. Write the description in plain words: what changed and why. Screenshots for anything visual.

## Plugins

The WebAssembly plugin host is gone. Luau plugins will attach to the session through `SessionHooks`, described in `docs/architecture.md`.
