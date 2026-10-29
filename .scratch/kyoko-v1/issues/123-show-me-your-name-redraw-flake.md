# 123 — Headless run's ShowMeYourName redraw check flakes

**What to build:** `cargo xtask headless-run --mock` fails now and then with `xtask: redrawing the timeline asked ShowMeYourName again` (`tools/xtask/src/headless.rs`, `formats_names_and_times`, the `ensure!` near line 409). On 2026-10-29 it hit all three lanes: lanes B and C once each during their full runs (passed on rerun), and the orchestrator's lane A run failed twice in a row with nothing else building, then passed on the third try. The same commit passed on its parent, so it is timing, not one change. The related desktop/bridge test `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one` is on the same load-flake list. Find out why a redraw sometimes asks the plugin again after the wait said it had settled: either the check is racing real state changes that arrive after it settles (late member chunks, presence or nick updates, a GUILD_MEMBER_UPDATE from mockcord), in which case the test must wait for what it really depends on, or kyoko's display cache really does re-ask for rows it already formatted, in which case fix kyoko. Say which in plain words.

**Blocked by:** none

**Status:** resolved

- [x] Root cause found and stated (test race or a real redraw bug), with evidence
- [x] `cargo xtask headless-run --mock` passes 15 runs in a row alone and 5 in a row while a workspace clippy build runs, on this machine
- [x] A deliberately broken display cache (one that re-asks the plugin on every redraw) still fails the check
- [x] If `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one` shares the cause, fix it the same way and say so; otherwise leave it and say why

## Answer

Merged `68f6af8` Check display redraws against one frozen state (lane A, worker `a11d4fb0`), verified by the orchestrator: all done-means steps green, plus 3 extra headless runs green. No UI change.

- Root cause: a test race, not a kyoko bug. The check counted every display request, and `pending()` included PlatformIndicators decorations keyed on presence. The 100 redraws read live state; a presence update arriving just after the wait (mockcord gives users who share guilds a different random presence per guild, kyoko keeps one per user, so presences flip as each guild's member data lands) made the redraw decorate those users. Name calls stayed at 52 throughout. Reproduced 1 in 30 runs under load with temporary diagnostics. The dont_round bridge test had the same kind of cause (redraw read member updates the view hadn't applied yet; 3 failures in 80 under load).
- Fix: bridge `Display::asks(Formatting)` counts requests per kind where they are sent; `pending_for(Formatting)`; `pending()` is their sum (Memo cache unchanged). xtask `redraws_ask_nothing` holds one state read guard, catches the timeline up once, redraws that state `REDRAWS` times and requires no new asks of that kind; used for ShowMeYourName, MessageLogger and PlatformIndicators. The dont_round test catches the view up under one held state before the RelativeTimes redraw.
- Boxes: headless 15/15 alone, 5/5 plus 30/30 under a cold clippy; a cache that re-asks fails 3/3; dont_round 100/100 under load, and a broken RelativeTimes redraw still fails it.
- Pattern for redraw-stability checks: redraw against one held `state.read()` and count asks of the kind under test. Candidate for the same treatment (not seen flaking): bridge `display::show_me_your_name_reaches_the_timeline_member_list_and_profiles`.
- Both entries removed from rules.md's flake list.