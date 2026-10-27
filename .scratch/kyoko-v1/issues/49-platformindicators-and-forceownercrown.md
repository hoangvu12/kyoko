# 49 — PlatformIndicators and ForceOwnerCrown

**What to build:** Users see desktop, mobile and web indicators on people, and the owner crown shows even in large servers.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 29 (Member list)

**Status:** resolved

- [x] User and member decoration slots exist
- [x] PlatformIndicators shows platforms from presence data
- [x] ForceOwnerCrown shows the crown regardless of server size

## Answer

Merged to main as `2c0c991`. Docs: `docs/plugins.md` (slot table, `decorate_user`, both plugins), `docs/parity.md` ("49"), `THIRD_PARTY_NOTICES.md`.

- Design (the v1 52 pattern): a new logic hook `decorate_user(user)` returns plain data: icon lists for `members`, `messages` and `profiles` (an SVG from the plugin's own folder, a theme-token colour, a tooltip) and a `crown` flag. It runs on the `display-hooks` thread through `bridge::Display` with a 50 ms deadline and falls back to plain names; answers are cached per person, server and presence (up to 4096), so redraws make no plugin calls. Drawn in the timeline and thread pane by `timeline/decorations.rs`, first in the profile badge row (popout and full profile), and in member rows from the same data (a `code:` SVG source), not a zgx slot per row: presence and ownership live in Rust and one cache serves all three.
- Grants: `ui:member.decoration` gives `members` icons and `crown`; `ui:user.decoration` gives `messages` and `profiles`; presence fields (`status`, `platforms`) only with `read:users`; the crown only ever lands on the server owner. Grant-gated targets go through `Inner::targets_where` and `PluginHost::formatters_where` (private to `host.rs`; add new grant-gated hooks in a child module like `host/decorations.rs`).
- PlatformIndicators (bundled, disabled): platforms from presence `client_status` (already decoded leniently); Lucide monitor, smartphone, globe in the person's status colour, tooltips like "Idle on Mobile", none for bots; settings for member list, messages, profiles, and colouring the mobile icon by status.
- ForceOwnerCrown (bundled, disabled): kyoko had no guild crown before; the stock rule (`bridge::shows_crown`) hides it when the server shows an Administrator role apart from online members (Discord's documented rule) or has more than 1000 members (kyoko's assumption; Discord documents no threshold). The plugin overrides both. Mockcord's hoisted Admin role hides the crown with the plugin off.
- Tests: plugin seam `crates/plugins/tests/plugins/decorations.rs` (grants and scopes, both plugins and settings, a failing or endless decorator leaves names plain and is suspended); bridge `crates/bridge/tests/bridge/decorations.rs` with the real host against mockcord (exact platforms per author and member, crown only on the owner, profile icons, about one call per person and none across 50 redraws, off clears everything); load check `ui::scripts::decorations`; `headless-run` runs both and reports `decoration_calls` (separate from `display_calls`, still 43–53). Mockcord spreads `client_status` over desktop, mobile, both and web by member index.
- Checks re-run by the orchestrator in lane B after rebasing onto 54 (union conflicts in `bundled.rs` (PACKAGES now 14), `docs/plugins.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md`, `headless.rs`): fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny all exit 0; luau0-src `0.22.0+luau740`. Kyoko-only captures (port 8496, checked by eye) `name-decorations` (platform icons in timeline and member list, owner crown) and `name-decorations-profile`; `member-list` with plugins off 14.3% against oxidcord, no icons or crown.
- Deviations: embedded (console) and VR platforms left out (kyoko decodes only desktop, mobile, web); your own icons come from your presence as Discord sends it, not your session list as Vencord does; with ForceOwnerCrown alone a presence change still triggers a plugin call.
