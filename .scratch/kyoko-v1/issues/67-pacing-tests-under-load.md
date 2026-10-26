# 67 — Pacing tests that hold under load

**What to build:** Make the session pacing tests stable when three lanes build at once. `community::forwards_are_paced_sends_and_respect_locks` failed in three of five full runs on 2026-10-26 (Midnight 11 and 23 verifications) and `friends::accepting_removing_and_blocking_are_paced_relationship_requests` failed once ("relationship requests 172.8ms apart"); both pass alone. They measure the gap between requests where mockcord receives them, so a request delayed in transit shrinks the next gap. Measure what kyoko controls instead (when the session releases each request, e.g. a pacer clock or a timestamp the session records), or give mockcord's arrival check a tolerance that only covers transit jitter, without weakening what the tests prove: kyoko never sends two paced requests closer than the gap.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Both tests (and any other test asserting gaps from mockcord arrival times) pass 20 runs in a row while two other lanes build
- [ ] A deliberately broken pacer (gap halved) still fails them
- [ ] The two tests leave the known-flakes list in the orchestrator's rules
