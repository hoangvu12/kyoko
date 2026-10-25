# Kyoko contributor rules

Kyoko is a lightweight native Discord client for Windows: a Rust core with a zgx window, a Luau interface and sandboxed Luau plugins. Read `docs/architecture.md` for the crate graph.

## Hard rules

- No code comments of any kind. Names and small functions carry the meaning. Docs live in `docs/`.
- Behaviour tests at the session seam, the session driven against an in-process mockcord, are required. Other unit tests only when a task explicitly asks for one. Also verify by running the real thing: `cargo xtask headless-run --mock` or the desktop app.
- No telemetry, analytics, crash upload, or remote logging. Never log the token, the Authorization header, or raw gateway payloads.
- Everything from the network, a plugin, or disk is untrusted. Lenient decoding: unknown enum values become `Unknown(i64)`, extra fields are ignored, a bad element never fails the whole payload.
- `protocol`, `rest`, `gateway`, `state`, `store`, `markdown` have no UI dependency and must build headless.
- The UI never touches the network. It reads `state` snapshots and sends `Command`s to the session.
- Tabs for indentation, `cargo fmt` before every commit, `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- Present the identify properties and REST headers of the Discord web client, nothing more; never automate user actions, bypass captchas, MFA, rate limits or client verification.
- Clean-room: other open source Discord clients may be read for protocol facts and ideas only. Never copy their source.

## Versions

Pinned in the workspace `Cargo.toml`. Rust 1.99.0 via `rust-toolchain.toml`. Check crates.io before adding a dependency and add it to `[workspace.dependencies]`, not a crate manifest.

## Commit style

Subject under 60 characters, imperative, no period. Optional plain body. No emoji, no bullet walls.

## Research

Background for kyoko (zgx + Luau UI on the forked core) lives in `docs/research/`. Start with `docs/research/README.md`.

## Agent skills

### Issue tracker

Issues and specs live as local markdown under `.scratch/<feature-slug>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical roles, label string equal to role name (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
