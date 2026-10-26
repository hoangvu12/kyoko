# 48 — ShowMeYourName and DontRoundMyTimestamps

**What to build:** Usernames can show next to nicknames or replace them, and relative timestamps round down.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 26 (Message rendering)

**Status:** resolved

- [x] A display-name formatting hook exists, and ShowMeYourName uses it
- [x] A relative-timestamp formatting hook exists, and DontRoundMyTimestamps uses it
- [x] Both apply across the timeline, member list and profiles

## Answer

Merged to main as `074ef0a`. Docs: `docs/plugins.md`, `docs/ui.md`, `docs/architecture.md`, `docs/parity.md`.

- Hooks: `hook:display_name` runs `format_name(person)` returning a string or `{ name?, extra? }` (`extra` drawn dimmed after the name); `hook:timestamp` runs `format_relative(time)` with `{ seconds, past, unit, value, text }` (`unit` the largest that fits, `value` the exact count, `text` kyoko's wording). Each has its own permission and a 50 ms deadline (`name_deadline`, `time_deadline`), chains in plugin id order, fails open (kyoko's text) and counts a strike. Interface `session::DisplayHooks` beside `SessionHooks`, implemented by `PluginHost`; the session never calls it.
- Cache `bridge::Display`: the UI thread only reads it; a miss draws kyoko's text and queues the question for a `display-hooks` thread; answers arrive as one wake per batch and redraw only changed rows (`apps/desktop/src/ui/display.rs`). Names keyed by user, server and the three name strings; relative times by the signed span. Dropped whenever the set of formatting plugins changes (enable, disable, suspend, restart, reload, or a settings change: `PluginHost::set_setting` bumps a per-plugin revision); old answers stay until new ones arrive; turning the last one off restores kyoko's names at once. Owners keep their `Display` across resets (`MemberPanel::reset`).
- Surfaces (muted part uses `text_muted`): timeline and thread pane authors, member list rows (Luau `row.extra`; the name has `shrink = 0` so only the username truncates), profile popout and full profile (`ProfileCard::name_extra`), `<t:..:R>` in message bodies. Reply previews, search, pins, forward snippets, system lines, reply authors, mentions and search results keep kyoko's text.
- ShowMeYourName: "nickname then username" (default), "username then nickname", "username only"; "Hide the username when it matches" on by default, case-insensitive. Vencord's "in replies" and friend-nickname switches left out. DontRoundMyTimestamps floors the count. Kyoko's own relative times now round to the nearest unit (as Discord does) and gained months and years.
- Calls: a full mock session makes 43–53 display calls (about one per distinct author/nickname plus a time or two); none while scrolling (10 passes measured) or re-projecting a channel 100 times. `headless-run` reports `display_calls`.
- Tests: plugin-seam tests for both hooks (permission, every mode, hide setting, fail-open, suspension, rounding), bridge tests with the real host against mockcord (timeline authors, member panel incl. after reset, profile card; only rows with relative times redrawn; a slow plugin never holds a lookup), load-check member row with `extra`. `bridge` has `kyoko-plugins` as a dev-dependency. Checks re-run by the orchestrator in lane C.
- Parity (port 8493): plugin off timeline 6.2%, member-list 6.2%, profile-popout 7.4% (popout pixel-identical to main; the others differ only by the hover toolbar under the shared mouse pointer); ShowMeYourName on 6.8% / 6.8% / 7.9%. New capture-only variable `KYOKO_CAPTURE_PLUGINS=<id,...>` enables bundled plugins in-process; kyoko signals ready once `Display::pending()` is 0.
- For 49/52: add a `Formatting` kind, a `formatters()` entry and a memo in `bridge::Display`, redraw from the wake in `ui/display.rs`; key on the inputs, never call `PluginHost` from a projection or render.