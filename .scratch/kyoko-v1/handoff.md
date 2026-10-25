# kyoko v1 orchestrator handoff

## Latest: moving to another machine (main `4c7338a` + this note)

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
