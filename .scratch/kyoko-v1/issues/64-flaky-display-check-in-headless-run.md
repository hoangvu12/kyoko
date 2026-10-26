# 64 — Flaky display check in headless-run

**What to build:** `cargo xtask headless-run --mock` passes reliably again. Ticket 48 added a step (`formats_names_and_times` in `tools/xtask/src/headless.rs`) that fails about half the time on main with "redrawing the timeline asked ShowMeYourName again".

**Blocked by:** 48 (ShowMeYourName and DontRoundMyTimestamps)

**Status:** claimed

- [ ] The cause is found and stated: either the check is wrong (a legitimate cache miss, e.g. member data arriving late and changing the cache key) or `bridge::Display` really asks the plugin again on a redraw
- [ ] If the cache is at fault, it is fixed; if the check is at fault, it checks the real property (no plugin calls per redraw once inputs are stable) without waiting on timing
- [ ] `cargo xtask headless-run --mock` passes 10 runs in a row, including runs while other lanes build