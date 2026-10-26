# kyoko v1 orchestrator handoff

## Fourth orchestrator (roboco chat `bde24e85`), running notes

- 34 resolved (`3837ceb`). The shared `oxidcord-ref` was not reset: 41's worker has an uncommitted `account-switcher` capture step there, so 34's one-line `SETTLE` wait was added by hand on top and the shared exe rebuilt. After 41 lands, `git -C ..\oxidcord-ref diff` should equal `tools/parity/oxidcord-ref.patch`.
- A second effort landed on main: `.scratch/kyoko-midnight/` (tickets called M01–M24 here), spec `docs/design/midnight.md`, mockups `docs/design/mockups/`. It restyles kyoko to Discord's layout in refact0r's Midnight theme; behaviour stays with the v1 tickets.
- User decisions (2026-10-26):
  - Scheduling: Midnight foundation now. Free lanes take M01 (tokens and Inter; after 57 lands, both touch the theme code) and M02 (mockup capture tooling), then M03 (floating-panel shell) and M04 (Twemoji). The third lane keeps doing v1 behaviour tickets. Restyles M05 and later come after the shell lands.
  - Look target: v1 UI tickets that haven't started (42, 43, 44, 62, 63, the slot-based plugins 49/52/54/56, 39) match the Midnight mockup where one exists, checked with M02's tooling, once M01–M03 have landed. Until then v1 tickets with UI wait or build behaviour only. Tickets in flight (41, 47, 57) finish to oxidcord's look.
- User decision (2026-10-26): nobody uses kyoko yet, so no backward compatibility, migrations or shims; change formats and defaults directly (also in `rules.md`).
- Disk: on 2026-10-26 C: filled to 0 bytes; lane targets had grown to 90–124 GB each (incremental dirs, test binaries, PDBs, captures that build an older commit in the lane). Check `Get-PSDrive C` before launching a worker; `cargo clean` an idle lane whose `target` passes about 60 GB (sccache makes the rebuild cheap). Never clean a lane while its worker is building.
- 57 was told its CSS import must recognise Midnight's variable names (one mapping table) so M01 can make the mapping one-to-one.

## Latest: third orchestrator hands off (main `85cd454` + this note)

Written by the third orchestrator (roboco chat `6e78591a`, "Kyoko Orchestrator Handoff", on the new PC) when the user asked to hand off. Everything below this section still applies unless this section says otherwise.

### State

- Resolved on this machine: 60, 37, 30, 31, 45, 32, 33, 35, 61 (new, click regression), 36, 38. Resolved overall: 01–33, 35–38, 40, 45, 59, 60, 61.
- In flight (completion notices go to `6e78591a`, not to you: send each running worker a short message so its next turn reports to you, or poll with `get_chat`/`read_chat`):

| Ticket | Lane | Worker chat | State |
|---|---|---|---|
| 34 friends | A | `5aae230b` (finished) | Worker done. The orchestrator rebased `lane-a` onto `73b91a0` (union conflicts in `capture.rs`, `docs/ui.md`, `state.luau`) and re-ran checks: fmt, clippy, build, `test --workspace` and `headless-run` passed; `xtask test` failed only in 6 `kyoko-plugins` tests ("the plugin took too long to load") under heavy load from the other lanes, and all 13 plugin tests passed when re-run alone; a friends capture was still running when this note was written (log `kyoko-lane-a/target/orch-verify.log`). Finish: re-run `cargo xtask test` once on a quieter machine, `git rebase origin/main` in lane A (main only gained `.scratch` commits), `git merge --ff-only lane-a`, push, write 34's Answer (worker report in `5aae230b`). Its commit `969b81e` changes `tools/parity/oxidcord-ref.patch` (oxidcord's friends capture waits for the DM to settle): after merging, re-apply the patch in `oxidcord-ref` (`git -C ..\oxidcord-ref checkout -- . ; git -C ..\oxidcord-ref apply --whitespace=nowarn <patch>`, check `git status` first, nobody else should have edits there) and rebuild with the cmake env vars below. Until then `friends-*` oxidcord captures show a DM; the worker's private build is `kyoko-lane-a/target/oxidcord-build/debug/oxidcord.exe` (`--oxidcord <exe>`). |
| 41 multi-account | B | `8982c463` (running) | Per-account databases, `Flow::switch`, accounts view in `profile/status_menu.rs`, second mock account; has a WIP commit `39bec48`. Told about 38 (`Link::reset_connection()` on the switch path, toasts) and that 34 adds `FriendsPage::reset()`. |
| 47 SilentTyping + plugin UI slots | C | `1e56e571` (running) | Builds the plugin UI-half loader and the `composer.buttons` slot that 49/52/54/56 reuse. |

- Unclaimed, in suggested order: lane A → 57 (rewritten, see decisions), then 62 (moderation menu follow-ups) and 63 (friend confirmations). After 41 lands: 42 QR login, 43 web login, then 44 (needs 43). Plugins 46, 48–56 (after 47 for the slot-based ones: 49, 52, 54, 56). 39 translations deliberately deferred until few UI tickets are in flight (it touches every string and conflicts with everything). 58 packaging last; any release or tag needs the user's go-ahead.

### Decisions the user made this session

- Ticket 57: no signing key and no catalog server. Themes come from bundled ones, local files in `<data>/themes/`, or links the user adds (like Vencord's Local/Online Themes); a theme is a kyoko `theme.json` or a Vencord/BetterDiscord `.css` whose colour variables are imported. Spec lines (story 84, Themes section) and ticket 57 are rewritten; remove the signed-catalog leftovers (`session/src/catalog.rs`, `xtask theme keygen/sign`, `themes/README.md` catalog section).
- Ticket 34 friends: "do the same as Discord": no kyoko lock on friend requests; Remove Friend and Block get Discord's confirmation dialogs (ticket 63).
- Tickets 34 and 44 are in scope (they were missing from the user's list by accident).
- sccache set up on this machine (user asked).

### This machine

- i9-12900HK (14 cores, 20 threads), 32 GB RAM, RTX 3070 Ti laptop, display at 150% scale (captures are 1920×1200 on both apps; shares still compare). C: about 125 GB free.
- Paths: main checkout `C:\Users\HP MEDIA\Desktop\nguyenvu\kyoko`; lanes `kyoko-lane-a`/`-b`/`-c` beside it; oxidcord reference `nguyenvu\oxidcord-ref`. Worker rules and done-means list (paste or point to in every prompt): `C:\Users\HP MEDIA\Desktop\nguyenvu\.orchestrator\rules.md` (outside the repo).
- 3 lanes × `CARGO_BUILD_JOBS=3`; incremental builds 20–40 s, machine stays responsive. Parity ports A 8494, B 8496, C 8493.
- sccache 0.18 (winget, `%LOCALAPPDATA%\Microsoft\WinGet\Links\sccache.exe`), set as `rustc-wrapper` in `~/.cargo/config.toml`; config `%APPDATA%\Mozilla\sccache\config\config` with `basedirs` = main checkout and the three lanes, 28 GiB disk cache. `cargo-deny` installed in `~/.cargo/bin`.
- No cmake on PATH. oxidcord builds need `$env:CMAKE='C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe'` and `$env:CMAKE_GENERATOR='Visual Studio 17 2022'`. `tools/parity/oxidcord-ref.ps1` aborts under PowerShell 5.1 on rustup's stderr ("syncing channel updates") because of `$ErrorActionPreference = "Stop"`; build the checkout with `cargo build -p oxidcord` directly (a small fix to the script is worth doing in some ticket).
- `KYOKO_TOKEN` is set as a Windows user environment variable (the user's throwaway token). kyoko starts a real session whenever it is launched without `--mock`, so workers always use `--mock`. No real-Discord run was made on this machine. Same rules as before: one at a time, look only, stop on captcha/401/fatal close; clear the variable when the user says it's revoked.

### How this session worked (lessons)

- Verification per ticket: review the report, check the diff for comments/conflict markers/manifest changes, run fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock` in the lane in the background (log to `target/orch-verify.log`), look at one `pair.png`, then ff-merge, push, Answer, claim next, fast-forward the idle lane, launch the next worker, and steer the running workers with "main moved to X (what landed), rebase before final checks".
- When two lanes finish on the same base, merge one and rebase the other yourself if conflicts are mechanical unions (both sides appended rows/variants/fields); verify again before merging. Anything semantic goes back to the worker. The main checkout gains `.scratch` commits between merges, so rebase a lane onto `origin/main` before `merge --ff-only`.
- Ticket 61: a closed root-overlay layer from 31 silently swallowed every click; none of the checks caught it. The rule and test are now in `docs/ui.md` ("Root overlay layers") and `ui::root`. Tell every UI worker about it (rules.md does).
- New load flake: 45's plugin tests (`crates/plugins/tests/plugins/`, `deadlines::*`, `storage::*`, `permissions::*`, `events::*`) fail with "the plugin took too long to load" when three lanes build at once; they pass alone. Add it to the known-flakes line in `rules.md`, and consider a small ticket to give tests a more generous load deadline (`Limits`) so the suite is stable under load.
- Workers on Claude can hit the 5-hour usage limit mid-turn; their transcript shows "Claude usage limit reached". Resume with `send_message` (mode run) describing where they stopped. "Stopped" completion notices were sometimes misleading (the worker was still running or had already finished): check `get_chat` before acting.
- The user wants short status notes, asks "how many tickets left" now and then (count from the `Status:` lines), and makes spec decisions quickly when asked plainly.

### Real-Discord confirmations still open (added this session)

- 35: the `X-Context-Properties` shape on joins (undocumented), `inputValue` on invite lookup, `flags` on invite creation, the join body outside guest invites. Joins are actions; don't test them on the user's account without asking.
- 34: whether the web client sends a location context on accept/remove/block, and `PUT /users/@me/relationships/{id}` for a profile's Add Friend.
- 33: thread request `location` values. 32: GIF category previews arrive as `.mp4` on real Discord.

## Earlier: moving to another machine (main `4c7338a` + this note)

The second orchestrator (roboco chat `02ecdad4`) stopped every worker and pushed everything to GitHub when the user moved the work to another PC. The worker chats and lane worktrees below live on the old machine only; on the new machine, recreate the lanes from the pushed branches and start fresh workers.

- Resolved since the section further down was written: 21, 23, 24, 26, 27, 28, 29. Read their `## Answer` sections; they carry the hand-off notes for later tickets.
- New ticket 60 (Luau load check), added because a rebase silently broke `ui/actions.luau` and no check noticed. Until 60 lands: after every rebase, launch the app or run one parity capture to confirm the UI script loads.
- In flight, all claimed on main, all unverified. Each branch sits on top of main, so `git log main..lane-x` shows its work:

| Ticket | Branch on origin | State |
|---|---|---|
| 30 user panel, status, profiles | `lane-a` | `5f9a5ec` (status saved in the settings proto, profile cache) plus WIP `c0b3021` (Rust profile layer, half done, not built). The popout, full profile and custom status dialog are still to do; then route 28's `TimelineView::on_profile` and 29's `Link::open_member_profile` to one profile entry point. |
| 37 settings pages | `lane-b` | `f04353c` "Add the settings and account pages": the worker said tests passed and it was finishing docs, then rebasing onto main with 29. Rebase, finish docs, run the full checks and parity (`settings-account`, `settings-appearance`). |
| 60 Luau load check | `lane-c` | WIP `bc415c6` (`ui/scripts.rs`, an `xtask` check), not built. The worker had found that zgx-script exposes `testing::TestApp`, which mounts a `ScriptHost` without a window. |

- To recreate a lane: `git worktree add ..\kyoko-lane-a lane-a` (after `git fetch`), and the same for b and c. Rebase each onto main before continuing, and squash or reword the WIP commits before merging.
- Frontier after these: 31 (quick switcher and search; uses 28's `TimelineView::jump_to`), 32, 33, 35, 36, 38, 39, 41, 42, 43, then 45 once 37 lands (45 needs 27 and 37), then the plugin tickets.
- Machine-specific items that don't travel: `KYOKO_TOKEN` (a user environment variable on the old PC; the new PC has none, so don't run against real Discord until the user sets one), the sccache `basedirs` config, the `rustc-slots` wrapper in the global cargo config, and the oxidcord reference build (recreate with `tools/parity/oxidcord-ref.ps1`).
- Lessons from this run:
  - All lanes share one desktop. Prefer driving the app in-process (zgx `Ui::dispatch`, as 28 did) over OS-level clicks. A worker that does use OS input must target its own window, close any dialog it opens, and only stop processes whose path is inside its own worktree; one worker's clicks once landed in another lane's dialog, and another worker's helper once killed every kyoko process on the machine.
  - Give each lane its own parity port (A 8494, B 8496, C 8493).
  - The oxidcord reference sometimes fails a capture in under a second when one runs right after another; rerun that screen alone.
  - Three lanes with `CARGO_BUILD_JOBS=5` plus the orchestrator's verification runs made the old PC laggy. On a similar machine, try `CARGO_BUILD_JOBS=3`, or two lanes.

## Earlier handoff (first orchestrator)

Written by the first orchestrator chat (roboco chat `6b0a1fd1`, "kyoko orchestrator") when the user asked to hand off. Read this, then `AGENTS.md`, `.scratch/kyoko-v1/spec.md`, `.scratch/kyoko-v1/execution.md`, `docs/agents/issue-tracker.md`, and the ticket files.

## The job

You are the orchestrator for kyoko v1: implement every ticket in `.scratch/kyoko-v1/issues/` by delegating each ticket to a worker side chat, and drive the work to completion. You coordinate, review and merge; you don't write feature code.

- **Delegate:** one worker side chat per ticket, created with roboco `create_chat` (`kind: side`, `harness: claude-code`, `cwd` = the lane worktree, `wait: false`), then end your turn and wait for the completion notice. Read results with `read_chat` (limit 1, `include_tools: false`), answer waiting workers with `respond_to_input`, send follow-ups with `send_message`.
- **Lanes:** three persistent worktrees beside the main checkout, reused across tickets, never a fresh worktree per ticket, never switch branches in the main checkout:
  - `C:\Users\ADMIN\Desktop\nguyenvu\kyoko-lane-a` (branch `lane-a`): always the critical chain (… → 25 → 26 → 28; 26 is in flight).
  - `kyoko-lane-b` (`lane-b`), `kyoko-lane-c` (`lane-c`): the frontier, lowest unblocked unclaimed ticket first; prefer tickets in different areas to limit conflicts.
- **Claim / resolve:** set `**Status:** claimed` on main before launching; on completion check the boxes, set `**Status:** resolved` and append an `## Answer` (what was done, merged commits, what later tickets must know). Status edits go on main, committed and pushed.
- **Worker prompt must include:** ticket path; the `## Answer` sections of the tickets it builds on (name the key facts); relevant spec and research sections; lane worktree and branch; `CARGO_BUILD_JOBS=5`; the rules below; "done means" = every acceptance box, fmt, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --locked`, `cargo test --workspace --locked`, `cargo xtask test`, `cargo xtask headless-run --mock`, `cargo deny check`, `luau0-src` still `0.22.0+luau740`, and for UI tickets a parity capture checked by eye; commit on the lane branch in AGENTS.md style; reply with summary, commits, deviations and notes for later tickets. Also tell workers: wait for builds inside their turn (short blocking waits), don't edit `.scratch/`, don't push. Past prompts in this chat's history are good templates (see the roboco transcript of `6b0a1fd1`).
- **Rules every worker follows:** `AGENTS.md` is law (no code comments of any kind incl. Luau, tabs, no telemetry, never log token/Authorization/raw payloads, lenient decoding, UI never touches the network); behaviour tests at the session seam are required; clean-room (concord GPL-3.0 and Vencord GPL-3.0+ are behaviour references only; oxidcord MIT/Apache and zgx MIT may be reused); UI tickets follow `docs/ui.md` and keep edits to shared files (`ui/state.luau`, `ui/actions.luau`, `src/ui.rs`, `src/capture.rs`, `docs/parity.md`, `docs/ui.md`) small and append-only; new deps in `[workspace.dependencies]` only when needed.
- **After a worker finishes:** review its report (and diff if anything looks off), run the checks yourself in the lane worktree (in the background with `timeout` up to 3600000), rebase onto main, fast-forward main (`git merge --ff-only lane-x` in the main checkout), push `origin main`, write the ticket answer, then fast-forward the other idle lanes. If it falls short, send the worker back with specific fixes rather than patching it yourself.
- **Ask the user first** for spec-level decisions and anything outward-facing beyond pushing kyoko's `main` (releases, tags, upstream bug reports, contacting anyone), or destructive outside the lane worktrees. Keep the user posted with short progress notes.

## Current state (main `771b4fa`, pushed)

Resolved: 01–20, 22, 25, 40, 59. In flight (all launched by the old orchestrator, so their completion notices go to `6b0a1fd1`, not to you — send each a short message so their next turn reports to you, or poll with `read_chat`/`get_chat`):

| Ticket | Lane | Worker chat | State |
|---|---|---|---|
| 21 token login, ToS, sign-out | B | `672aacf5` "kyoko 21 token login (lane B)" | Verified green on its old base (login-tos 2.3%, login-token 2.4%). Sent back to rebase onto main after 22: conflicts in `capture.rs`, `ui.rs`, `docs/ui.md` are mechanical; the real fix is `apps/desktop/src/ui/sign_in.rs:194`, which clears `self.guilds` that 22 removed (clear the rail through `apps/desktop/src/rail.rs`). |
| 23 channel sidebar | C | `a70ed9f2` "kyoko 23 channel sidebar (lane C)" | Launched on `771b4fa`. |
| 26 message rendering | A | `8641f671` "kyoko 26 message rendering (lane A)" | Large ticket; told that 22 landed and to rebase before final checks. |

Unblocked and unclaimed right now: **24** (DM sidebar), **27** (composer; 25 and 09 done), **29** (member list), **30** (user panel, status, profiles), **37** (account and settings pages), **38** (toasts, transitions, connection status), **39** (translations), **58** (Windows packaging — building is fine; any release/tag is outward-facing, ask first). After 21: 41, 42, 43 (then 44). After 26: 28 (critical chain; also needs 10, done), 32, 33. After 23: 35, 36. After 24: 34. After 28: 31. Plugins 45 (needs 27 and 37) then 46–56; 57 after 37.

## How things are wired (read the ticket answers for details)

- Core: session test harness (05), gateway policy (07), fingerprint (06, 59), capabilities 509 (08), REST safety layer (09), paced acks / safe deletes (10), risky-action locks (11), bounded caches (12), client_state versions (13), member lists + op 8 (14), store hardening + generic sign-out wipe (15), kept deleted/edited messages (16), backfill (17), media gallery (18).
- UI: zgx pinned at `fc7d0f1e6695b2153bc0e18f0f1a3cef6f0426f6` (zgui was renamed zgx; user decided to move). Bridge crate `crates/bridge`, desktop host `apps/desktop`, Luau in `apps/desktop/ui/`, conventions in `docs/ui.md`. The Rust timeline (25) sits above the whole Luau tree: overlays over the message area must be drawn in Rust or hide the slot (noted on 28, 30–33, 35–38). Parity tooling (19): `cargo xtask parity capture --screens …`, oxidcord reference build at `C:\Users\ADMIN\Desktop\nguyenvu\oxidcord-ref` (recreate with `tools/parity/oxidcord-ref.ps1`), screen list in `docs/parity.md`.
- Desktop integration (40): tray, toasts, hotkey, autostart, single instance, deep links.

## Environment and gotchas

- Machine: i5-14400F, 32 GB RAM, C: about 90 GB free now (user freed space; floor 40 GB — if below, clean an idle lane's `target/debug/incremental` or `cargo clean` an idle lane and drop to 2 lanes). sccache basedirs include the kyoko checkout and all three lanes (`%APPDATA%\Mozilla\sccache\config\config`); global cargo config uses the `rustc-slots` wrapper with `jobs = 8`, overridden by `CARGO_BUILD_JOBS=5`. sccache sometimes drops connections mid-build: retry.
- PowerShell 5.1: `Set-Content -Encoding utf8NoBOM` doesn't exist; write files with `[IO.File]::WriteAllText(path, text, (New-Object Text.UTF8Encoding $false))` or the Edit/Write tools. Ticket files contain `—`; a plain `Get-Content | Set-Content` round trip mangles it. Always Read a ticket file before Write (they change on main).
- `cargo tree -i luau0-src` exits 255 in pipelines even when fine; read its output, not the exit code.
- Parity captures from two lanes at once collide on mockcord port 8485; rerun or pass `--port`.
- Test timing: the session suite takes 10–50 s depending on load; a few tests time out occasionally under three parallel builds (07's send-window test was fixed; `capabilities::a_refreshed_token_reaches_rest_the_gateway_and_the_store` flaked once). Re-run in isolation before blaming a ticket.
- Rebase conflicts between lanes are frequent in shared desktop files. Mechanical unions (both sides added a field/variant/import/doc row) can be resolved by the orchestrator; anything semantic goes back to the worker.
- Workers sometimes end their turn while a background build runs; if a completion notice shows unfinished work, send them back to wait for the build within their turn.

## Real Discord and the user's machine

- `KYOKO_TOKEN` is set as a Windows **user** environment variable (the user pasted their throwaway token into chat and said "just use it, I will revoke later"). Workers read it with `[Environment]::GetEnvironmentVariable('KYOKO_TOKEN','User')` and pass it only to the app process; never print/log/commit it or put it in a prompt. Real runs: one at a time, never parallel, look only, no actions, stop on captcha/401/fatal close; the real path uses `client_identity(&store)`, `build_source: Some(discord_build_source())`, `remember_account: false`. When the user says they revoked it, stop real runs and clear the variable (`[Environment]::SetEnvironmentVariable('KYOKO_TOKEN', $null, 'User')`).
- Real-Discord confirmations still open: a live message arriving (04; ask the user to post during a run), fingerprint details listed in 59's answer, that 22's settings-proto PATCH with only `guild_folders` leaves other settings intact, whether READY ever carries `auth_token` (08).
- Leftovers on the user's machine (told to the user): Credential Manager entry `cache-key.kyoko` from early test runs (safe to delete, user's call); registry key `HKCU\Software\Classes\AppUserModelId\io.github.hoangvu12.Kyoko` (needed for toasts). Windows notifications are off machine-wide on the dev box (`ToastEnabled = 0`), so toast banners can't be seen.
- Pending outward-facing items to ask the user about: reporting the zgx exit-hang to upstream (repro and suggested fix in `docs/spike.md`); bumping Chrome to 157 after 2026-11-03 (a small follow-up ticket); any release from ticket 58.
