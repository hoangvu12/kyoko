# Contributing

Read `AGENTS.md` first. It holds the rules every change follows.

## Setup

Rust 1.99 is pinned by `rust-toolchain.toml`. Kyoko v1 builds on Windows only and needs the MSVC toolchain and cmake.

```bash
cargo run -p kyoko -- --mock
```

## Before opening a pull request

```bash
cargo xtask check-all
cargo xtask headless-run --mock
```

## Pull requests

Keep them small and on one topic. Write the description in plain words: what changed and why. Screenshots for anything visual.

## Plugins

Plugin API changes are additive only. See `docs/plugin-api/`.
