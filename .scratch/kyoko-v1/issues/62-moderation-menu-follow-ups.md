# 62 — Moderation menu follow-ups

**What to build:** Three small fixes found while verifying 36 in the running app.

**Blocked by:** 36 (Moderation and server settings)

**Status:** resolved

- [x] The Roles submenu opened from a member-list row near the right edge of the window appears inside the window (flipped or clamped like 28's context menu), confirmed in the running app
- [x] Roles the member already has show a check in the Roles submenu, and choosing one removes it
- [x] Deleting the channel that is open moves the view to another channel of the server (as the web client does) instead of leaving the main column blank

## Answer

Merged to main as `36b4b31`. Docs: `docs/ui.md` (member menu paragraph), `docs/parity.md` (new "62" section).

- Submenu placement: zgx places a submenu against the monitor, not the kyoko window, so a menu at the window's right edge opened its submenu past the window. Fix in `moderation/menu.rs`: when there is no room for a second 220-point panel on the right, the trigger row is marked right-to-left (`.direction(Rtl)`), and zgx opens the submenu to the left; each submenu is capped to the height below its row and scrolls. Applies to Move To as well. `kit::menu_row`'s inner row is now pinned left-to-right, which is what lets any zgx submenu trigger be flipped this way.
- Held roles: zgx draws its own check only on menu items without children, so role rows never showed one. Role rows now use `kit::checkbox`; the current voice channel in Move To shows a check icon. The remove path was already right.
- Deleted open channel: `bridge::replacement_channel` picks the server's first visible text or announcement channel in sidebar order; `Link::apply` opens it when the open channel disappears (deleted from kyoko or elsewhere). Not checked against the live web client (open real-Discord confirmation: which channel the web client opens after deleting the open one).
- Tests: in-process `moderation::tests::the_roles_submenu_from_a_member_row_at_the_right_edge_stays_in_the_window` (default 1100×720 window, screen larger than the window; right edge, room on the right, a 30-role list from the bottom-right corner; fails without the fix: submenu at x=1087, list 1032 high) and `a_held_role_is_checked_and_choosing_it_removes_it`; session `moderation::choosing_a_held_role_sends_one_delete_and_the_member_loses_it` (one `DELETE /guilds/{g}/members/{u}/roles/{r}`, no body, no audit-log reason); bridge `guild_admin::deleting_the_open_channel_moves_the_view_to_the_servers_first_channel`.
- Capture: kyoko-only `moderation-roles-submenu` (run with `--apps kyoko`), driven through `Ui::dispatch` (right-click the member's name, hover Roles via `ModerationLayer::roles_trigger()`). Checked by eye by the orchestrator: submenu opens left of the menu, inside the window, Members checked. `member-list` parity 14.3%.
- Checks re-run by the orchestrator in lane C on `845287f`: fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, `deny` all exit 0; luau0-src `0.22.0+luau740`.
- Deviations: a very long role list near the bottom scrolls instead of shifting up as the web client does (zgx lines a submenu's top up with its row); in a left-opening submenu assistive technology sees the trigger as right-to-left (Right arrow still opens it).
