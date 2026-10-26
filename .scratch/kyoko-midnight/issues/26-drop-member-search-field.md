# 26 — Drop the member search field

**What to build:** Remove v1 29's "Search members" field from the top of the member list. Discord and `midnight-discord.html` have none (user decision, 2026-10-26: the design wins where it differs from a v1 behaviour). Remove the field, its filter state and actions, its load-check sample and capture steps; keep member list scrolling and op 8 subscriptions as they are.

**Blocked by:** 03

**Status:** resolved

**Mockups:** `docs/design/mockups/midnight-discord.html`

- [x] The member list starts with its first group header, as in the mockup capture (02)
- [x] No dead filter code or shared values are left behind; the load check and `ui::root` pass

## Answer

Merged to main as `a22beb4`. Docs: `docs/ui.md` (the list has no search field and starts with the first group header).

- Removed: the field, `members.query`, the 350 ms delay and `search()` in `shell/member_list.luau`; `member_search`/`member_search_remote` in `actions.luau`; `members.search`/`members.search_remote` and `search()`/`search_command()` in `apps/desktop/src/member_list.rs`; query state, `query()`, `set_query()`, `search_command()`, the "Search results - N" rows and `MemberListEntry::kind` in `crates/bridge/src/member_list.rs`. `flush` always builds from the member list; `set_viewport` always sends `SetMemberViewport` (it was suppressed while a query was active), so op 37 ranges and op 8 requests behave as before. `Command::SearchMembers` and the session scheduler stay (composer autocomplete and server settings use them).
- Scroll maths unchanged (row tops measured from 0, Luau offset relative to the list). The field had no shared values, load-check samples or capture steps.
- Checks re-run by the orchestrator in lane C: all green. Mockup parity (port 8493): member-list 17.7% (17.5% before; the rows now sit 40 points higher against different stand-in content); the first header lines up with the mockup's and rows are 44 apart with or without an activity line.
- Note: `MemberHeights::default()` still has the old group height (~47.4, harmless: Luau sends the real heights with every `members.scroll`).
