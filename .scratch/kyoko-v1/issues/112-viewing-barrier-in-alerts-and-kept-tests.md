# 112 — Viewing barrier in the alerts and kept tests

**What to build:** 111 found that the session loop picks at random between pending commands and gateway messages, so a test that sends `SetViewing(false)` and then posts through mockcord can have the post handled first under load. `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus` (`crates/session/tests/session/alerts.rs:316`) and `kept::a_deleted_message_stays_in_place_flagged_and_survives_a_restart` (`kept.rs:193`) do exactly that and are on rules.md's flake list. Add the `SetDraft` barrier 111 used (see `docs/testing.md`), and check the rest of the session and plugin tests for the same pattern (a viewing or focus command followed by a mockcord post with no barrier).

**Blocked by:** none

**Status:** claimed

- [ ] Both tests (and any others found) wait for a barrier after the viewing or focus change
- [ ] Each passes 20 runs alone and 10 during a full workspace build; breaking the behaviour still fails them
- [ ] The orchestrator takes them off rules.md's flake list
