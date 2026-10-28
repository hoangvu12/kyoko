# 111 — Favourites frecency test that holds under load

**What to build:** `favourites::a_favourited_emoji_is_read_then_written_to_the_frecency_proto_and_unfavouriting_removes_it` is on rules.md's known-flake list and failed three times on 2026-10-28 alone (100's workspace run, 110's `xtask test`, and an earlier lane run), each time passing alone. Find what it waits on with a fixed time or an arrival gap (as 67 and 109 did for pacing and alerts tests), make it wait for what kyoko actually does, and take it off the flake list. While there, look at `one_ping_per_dm::repeated_direct_messages_ping_once_until_read` (kyoko-plugins; timed out once in a harness wait at `harness.rs:310`, and its `quiet` helper asserts silence after a fixed 300 ms settle) and fix it the same way if it's the same kind of problem.

**Blocked by:** none

**Status:** resolved

- [x] Root cause in plain words for each test (what timing it relied on)
- [x] Each passes 20 runs in a row alone and 10 during a full workspace build; a deliberately broken behaviour (e.g. frecency never written) still fails it
- [x] Both leave the known-flakes list in the orchestrator's rules (the orchestrator edits rules.md)

## Answer

Merged to main as `52e5daf` and `da823cc` (worker `615bb697`, lane C; full verify re-run by the orchestrator: all green). Docs: `docs/rest.md` (echo handling), `docs/testing.md` (`SetDraft` barrier, hook-recording wrapper). Both tests leave rules.md's flake list.

- Favourites, a real kyoko bug: after each favourites save Discord echoes the saved list over the gateway; if the echo of the first save arrived after the user's next change, kyoko replaced the newer list with the old one and the delayed save then wrote it back. Now, while a favourites change is pending or being sent, kyoko reapplies its newest local list after any incoming update (as the status writer already did); covers favourite emoji, GIFs and frequently used commands (`crates/session/src/task/favorite_writes.rs`, called from `task.rs`). New test `a_late_echo_of_an_earlier_favourite_write_keeps_the_newer_choice` (fails without the fix); the original test also waits for the removal in state. Once nothing is pending, incoming updates replace favourites again, so a very late echo can briefly show an old list.
- One ping per DM (test only): the session loop picks at random between pending commands and gateway messages, so under load a new DM could be handled before `SetViewing(false)` and be marked read. `look_away` now sends `SetDraft` after `SetViewing(false)` and waits for it (as `reads.rs` does); a `Heard` hook wrapper records each notify answer, and `rings`/`quiet` wait for it instead of a fixed 400 ms pause. Cause read from the loop, not reproduced.
- Runs: 20/20 alone and 10/10 during a from-scratch workspace clippy build for both; deliberately broken behaviour fails each.
- For later: `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus` (`alerts.rs:316`) and `kept::a_deleted_message_stays_in_place_flagged_and_survives_a_restart` (`kept.rs:193`) send `SetViewing(false)` and post without a barrier; likely the same cause (ticket 112).
