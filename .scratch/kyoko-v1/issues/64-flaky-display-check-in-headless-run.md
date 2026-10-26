# 64 — Flaky display check in headless-run

**What to build:** `cargo xtask headless-run --mock` passes reliably again. Ticket 48 added a step (`formats_names_and_times` in `tools/xtask/src/headless.rs`) that fails about half the time on main with "redrawing the timeline asked ShowMeYourName again".

**Blocked by:** 48 (ShowMeYourName and DontRoundMyTimestamps)

**Status:** resolved

- [x] The cause is found and stated: either the check is wrong (a legitimate cache miss, e.g. member data arriving late and changing the cache key) or `bridge::Display` really asks the plugin again on a redraw
- [x] If the cache is at fault, it is fixed; if the check is at fault, it checks the real property (no plugin calls per redraw once inputs are stable) without waiting on timing
- [x] `cargo xtask headless-run --mock` passes 10 runs in a row, including runs while other lanes build
## Answer

Merged to main as `a3b388d` (only `tools/xtask/src/headless.rs`).

- Cause: the check, not the cache. After history loads, the session's backfill sends op 8 for message authors without a member; the old check stopped waiting as soon as names showed usernames, which could be before `GUILD_MEMBERS_CHUNK` arrived. Users were then asked again with their nickname during the 100 redraws, a legitimately new cache key. `bridge::Display` never re-asked for a key it already had.
- Fix: the step waits until every author member the mock has is in state (`members_settled`), `Display::pending()` is 0 and every plain timeline entry shows its username; then it records the count, redraws 100 times and requires zero new calls. The Display answer callback wakes the harness's `Notify`, so there are no sleeps or polls. `display_calls` is now a steady 53 (52 names + 1 time).
- `headless-run` passed 10/10 for the worker and 3/3 for the orchestrator.
- For later checks counting display calls: wait for member backfill first (`members_settled` + `Display::pending() == 0`); pass a waking callback to `bridge::Display::new` instead of polling.