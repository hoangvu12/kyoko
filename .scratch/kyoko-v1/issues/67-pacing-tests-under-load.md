# 67 — Pacing tests that hold under load

**What to build:** Make the session pacing tests stable when three lanes build at once. `community::forwards_are_paced_sends_and_respect_locks` failed in three of five full runs on 2026-10-26 (Midnight 11 and 23 verifications) and `friends::accepting_removing_and_blocking_are_paced_relationship_requests` failed once ("relationship requests 172.8ms apart"); both pass alone. They measure the gap between requests where mockcord receives them, so a request delayed in transit shrinks the next gap. Measure what kyoko controls instead (when the session releases each request, e.g. a pacer clock or a timestamp the session records), or give mockcord's arrival check a tolerance that only covers transit jitter, without weakening what the tests prove: kyoko never sends two paced requests closer than the gap.

**Blocked by:** none

**Status:** resolved

- [x] Both tests (and any other test asserting gaps from mockcord arrival times) pass 20 runs in a row while two other lanes build
- [x] A deliberately broken pacer (gap halved) still fails them
- [x] The two tests leave the known-flakes list in the orchestrator's rules

## Answer

Merged to main as `504b1d9..1443f65`. Docs: `docs/testing.md` (never assert gaps between mockcord arrivals; the helper), `docs/rest.md` (the release log).

- Pacing is now checked where kyoko decides it: `reserve` in `crates/rest/src/safety/pacing.rs` returns the release slot even when already due (the client still sleeps only for a future slot; production pacing unchanged), and the REST client keeps method, path and slot of the last 64 paced requests (`Session::releases()`, next to `Session::reconnects()`).
- Test helper `running.assert_released_apart(&requests, gap, what)`: slots at least the full gap apart (no slack) and each request reached mockcord no earlier than its slot. Switched to it: `community::forwards_are_paced_sends_and_respect_locks`, `friends::accepting_removing_and_blocking_are_paced_relationship_requests`, `command_options::typing_is_debounced_into_one_autocomplete_paced_like_other_interactions`, `threads::a_forum_post_waits_its_turn_behind_a_send`. `GAP_SLACK` and `PACED_AT_LEAST` are gone.
- Backfill tests (`replies_to_unloaded_messages_fetch_each_original_once_and_paced`, `unknown_mentions_in_a_dm_are_fetched_over_rest_and_paced`): their gap is the session's own backfill queue, not the pacer, so they check the k-th fetch arrives no earlier than trigger + k gaps (transit only delays).
- Storage flake was a real race: `AccountStore::save` writes the keyring secret before adding the account to the index, and the test only waited for the secret; it now also waits for `accounts.current()`. Proven with a temporary 300 ms sleep in that window (old wait fails, new passes; reverted).
- Evidence: the 7 fixed tests passed 20 of 20 runs with other lanes building (CPU 94–100%); with every pacer gap and the backfill gap halved all 6 pacing tests fail (reverted). Full checks re-run by the orchestrator in lane C: all green.
- Known-flakes list in the orchestrator's `rules.md`: `community::forwards_…` and `storage::turning_the_opt_in_…` removed. The rest stay (they time out on a state wait, no obvious timing cause found).
- For later tickets: new pacing tests use `assert_released_apart`, never gaps between arrivals. Remaining risk: `gateway.rs` checks heartbeat gaps between arrivals with 50 ms slack (the gateway timer, not the pacer); under heavy load it could fail the same way.
