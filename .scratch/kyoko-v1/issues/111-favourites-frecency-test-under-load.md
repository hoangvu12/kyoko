# 111 — Favourites frecency test that holds under load

**What to build:** `favourites::a_favourited_emoji_is_read_then_written_to_the_frecency_proto_and_unfavouriting_removes_it` is on rules.md's known-flake list and failed three times on 2026-10-28 alone (100's workspace run, 110's `xtask test`, and an earlier lane run), each time passing alone. Find what it waits on with a fixed time or an arrival gap (as 67 and 109 did for pacing and alerts tests), make it wait for what kyoko actually does, and take it off the flake list. While there, look at `one_ping_per_dm::repeated_direct_messages_ping_once_until_read` (kyoko-plugins; timed out once in a harness wait at `harness.rs:310`, and its `quiet` helper asserts silence after a fixed 300 ms settle) and fix it the same way if it's the same kind of problem.

**Blocked by:** none

**Status:** claimed

- [ ] Root cause in plain words for each test (what timing it relied on)
- [ ] Each passes 20 runs in a row alone and 10 during a full workspace build; a deliberately broken behaviour (e.g. frecency never written) still fails it
- [ ] Both leave the known-flakes list in the orchestrator's rules (the orchestrator edits rules.md)
