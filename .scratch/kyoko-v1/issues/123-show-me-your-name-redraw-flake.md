# 123 — Headless run's ShowMeYourName redraw check flakes

**What to build:** `cargo xtask headless-run --mock` fails now and then with `xtask: redrawing the timeline asked ShowMeYourName again` (`tools/xtask/src/headless.rs`, `formats_names_and_times`, the `ensure!` near line 409). On 2026-10-29 it hit all three lanes: lanes B and C once each during their full runs (passed on rerun), and the orchestrator's lane A run failed twice in a row with nothing else building, then passed on the third try. The same commit passed on its parent, so it is timing, not one change. The related desktop/bridge test `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one` is on the same load-flake list. Find out why a redraw sometimes asks the plugin again after the wait said it had settled: either the check is racing real state changes that arrive after it settles (late member chunks, presence or nick updates, a GUILD_MEMBER_UPDATE from mockcord), in which case the test must wait for what it really depends on, or kyoko's display cache really does re-ask for rows it already formatted, in which case fix kyoko. Say which in plain words.

**Blocked by:** none

**Status:** claimed (lane A)

- [ ] Root cause found and stated (test race or a real redraw bug), with evidence
- [ ] `cargo xtask headless-run --mock` passes 15 runs in a row alone and 5 in a row while a workspace clippy build runs, on this machine
- [ ] A deliberately broken display cache (one that re-asks the plugin on every redraw) still fails the check
- [ ] If `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one` shares the cause, fix it the same way and say so; otherwise leave it and say why
