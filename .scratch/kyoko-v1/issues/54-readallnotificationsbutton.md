# 54 — ReadAllNotificationsButton

**What to build:** One button marks every server as read.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 10 (Paced read receipts and safe multi-delete), 22 (Server rail and folders)

**Status:** resolved

- [x] A button slot exists near the server rail
- [x] Mark-all-read uses the paced ack path

## Answer

Merged to main as `93e2ce9`. Docs: `docs/plugins.md` (slot table "placed (54)", ReadAllNotificationsButton section), `docs/testing.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md`.

- No new slot or plugin API: a bundled plugin `plugins/read-all-notifications/` (ships disabled) asks for `action:mark_read` and `ui:rail.buttons`, no settings. `ui.luau` is one tile under Home styled like the rail's Add a Server tile, Lucide `check-check` (ISC), tooltip "Read All Notifications"; a click calls `plugin.read-all-notifications.action { key = "read_all", slot = "rail.buttons" }` and `on_action` runs `kyoko.actions.mark_all_read()` inside the gesture. Four files appended to `bundled.rs` (12th package). No confirmation and no toast (badges clearing is the feedback).
- Paced path: only `Command::MarkAllRead` → `POST /read-states/ack-bulk`, at most 100 channels per request, `bulk_ack_gap` apart; skips channels without View Channel, covers only channels behind or with mentions; DMs included (as `docs/rest.md` says).
- Tests: plugin seam `crates/plugins/tests/plugins/read_all_notifications.rs` (ships disabled with exactly those permissions; running or another key sends nothing; >100 unread channels give `ceil(n/100)` acks via `assert_released_apart`, never one per channel; a second click sends nothing; harness gained `Options::channels(n)`); session `mark_all_read_clears_badges_with_bulk_acks` now uses >100 channels and counts requests; in-process `the_read_all_button_on_the_rail_marks_everything_read_only_when_clicked` (no gesture, nothing sent; real click clears every badge; disabled leaves the rail as before); `headless-run` clicks it and reports `read_all_acks: 1`.
- Beyond the ticket: `headless-run`'s mockcord ran with recording off, so its wire checks (SilentTyping, hidden channels) saw nothing; recording is now on and they still pass. `enable_capture_plugins` waits until a plugin's UI half is mounted, so `KYOKO_CAPTURE_PLUGINS` captures don't miss it.
- Checks re-run by the orchestrator in lane C on `7ac5732`: fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny all exit 0; luau0-src `0.22.0+luau740`. Kyoko-only captures (port 8493): `server-rail-read-all` with `KYOKO_CAPTURE_PLUGINS=read-all-notifications` shows the tile under Home (checked by eye; without the variable it is absent, as intended), `server-rail` unchanged.
- For 56: same shape with `ui:panel.buttons` (`props.slot.area` is `"panel"`); `action:activity` is still reserved, so 56 adds the `Action` variant, the call and `Permission::available`; copy the session-plus-mockcord `TestApp` pattern from `mock_session` in `ui/scripts/plugins.rs`.
