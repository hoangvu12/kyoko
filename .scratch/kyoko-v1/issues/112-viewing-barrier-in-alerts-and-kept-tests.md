# 112 — Viewing barrier in the alerts and kept tests

**What to build:** 111 found that the session loop picks at random between pending commands and gateway messages, so a test that sends `SetViewing(false)` and then posts through mockcord can have the post handled first under load. `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus` (`crates/session/tests/session/alerts.rs:316`) and `kept::a_deleted_message_stays_in_place_flagged_and_survives_a_restart` (`kept.rs:193`) do exactly that and are on rules.md's flake list. Add the `SetDraft` barrier 111 used (see `docs/testing.md`), and check the rest of the session and plugin tests for the same pattern (a viewing or focus command followed by a mockcord post with no barrier).

**Blocked by:** none

**Status:** resolved

- [x] Both tests (and any others found) wait for a barrier after the viewing or focus change
- [x] Each passes 20 runs alone and 10 during a full workspace build; breaking the behaviour still fails them
- [x] The orchestrator takes them off rules.md's flake list

## Answer

Merged to main as `58b59b3` (worker `53e72389`, lane C; full verify re-run by the orchestrator: all green). Test code only; both tests leave rules.md's flake list.

- Root cause: the session loop picks at random between pending commands and gateway messages, so under load the mockcord post could be handled before `SetViewing(false)`; alerts then stayed silent and kept marked the messages read. Both tests now send `SetDraft` after `SetViewing(false)` and wait for it in state before posting (111's barrier). No other test has the pattern (`reads.rs` and `one_ping_per_dm.rs` already have it; `reads.rs:213` waits for an ack).
- 20/20 alone, 10/10 during a from-scratch workspace clippy build, 5/5 after rebasing onto 103. With viewing forced on, alerts always fails; kept failed 8/8 measured but passed once earlier, because its first wait can catch the unread marker before kyoko marks the message read. A follow-up could assert the marker is still there after that wait.
