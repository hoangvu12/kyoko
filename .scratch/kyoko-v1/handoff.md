# kyoko v1 orchestrator handoff

## Latest: twelfth orchestrator hands off (roboco chat `22c07f0d`, "kyoko orchestrator 12", same machine, main `6a41204` + this note)

Everything below still applies unless this section says otherwise. Handed off because my context grew large (2026-10-29).

### Decisions the user made this session

- **DAVE crypto on BoringSSL, "do like Discord"** (2026-10-28): libdave runs on OpenSSL/BoringSSL; kyoko keeps oxidcord's protocol code with its own openmls provider on btls. Done in 99; RUSTSEC-2026-0387 ignore removed.
- Nothing else is waiting on the user. Open offers to them: (1) launch kyoko on their account for **102, their first real voice call** (all of 98-101 and 100 are in; recipe in the tenth orchestrator's section; rebuild the main checkout first; look only), (2) a fix for push to talk stealing its key combination from other apps (100 uses a Windows hotkey; Discord uses a keyboard hook) if they want it, (3) whether to chase the HTTP/2 priority-chaining gap that 93 made more visible (`docs/fingerprint.md`).

### Merged this session (Answers written for each)

82 image grid, 84 plugin dialog, 98 voice join/panel/mute, 109 theme_links flake, 83 rail hover card and voice badge, 99 DAVE on BoringSSL, 85 hover text rule, 89 image viewer, 90 new logo (un1rock art, credited), 101 call view and speaking, 93 image fetch queue (one HTTP/2 connection per host, visible first), 94 voice row hover buttons, 100 Voice & Video settings, 110 media-queue test flake (Roboco's own probe), 111 favourites echo bug (real fix) and one-ping test, 103 camera video receive, 112 viewing barrier in tests, 106 Go Live viewing. New tickets 109-113 were mine.

### State now (all three lanes running; completion notices go to `22c07f0d`: poll with `list_chats` parent `22c07f0d` and `read_chat`, or send each a short note so its next turn reports to you)

- **Lane A** `kyoko-lane-a`: **113** stream audio and full-window stream viewer, worker `d9d6b49d`, base `6a41204`.
- **Lane B** `kyoko-lane-b`: **104** camera send, worker `1e626948`, base `ab7bf65`; told about 106 and that 91 moves the MF decoder; must rebase onto 106 (tiles focus by `key`, `video_surface.rs` two lanes).
- **Lane C** `kyoko-lane-c`: **91** inline video, worker `5d0cc41b`, base `7893d58`; first commit moves MF decode/convert out of `crates/voice/src/video/` into a shared crate (one decoder for calls and messages).

### What's left

- Running: 104, 113, 91.
- Then: 105 screen capture and Go Live picker (after 104), 107 going live (after 105 and 106). 74 part 2b Chrome 157 bump after 2026-11-03. 75 crash needs a real run with stderr. 102 is the user's real call (`ready-for-human`). 108 DM calls (`needs-triage`). Skipped: 39, 58.
- Small leftovers: libopus (BSD-3) missing from `THIRD_PARTY_NOTICES.md` (from 97); 112's note on the kept test; 103's note (MF hardware decode + an NV12 zgx surface would cut most video CPU).

### Gotchas this session

- Workers end their turn while checks run almost every time: when a "finished" notice has a short last message ("waiting on X"), send "keep going in this turn, poll until done, then report". My queued "main moved" notes also fire "finished" notices; read one message further back.
- When I verify a lane, tell its worker "don't rerun anything, reply ok" first, or its own checks collide with mine in the same target.
- Roboco's desktop app probes newly opened localhost ports (plaintext `GET`/`HEAD /`): any test counting connections or requests on a fresh listener must count only its own (109, 110).
- Flake list in rules.md was trimmed (favourites, one_ping, alerts::channel_in_view, kept removed; composer::a_server_gate added). sccache dropouts still happen: rerun with `verify-rest.sh`.
- Usage limits stopped workers once for ~3 h; resend "resume" after.
- Disk: ~110-135 GB free after cleaning; clean a lane's `target` right after its merge when the lane goes idle.

## Latest: eleventh orchestrator hands off (roboco chat `8e102e63`, "kyoko orchestrator 11", same machine, main `c4ad329` + this note)

Everything below still applies unless this section says otherwise. The user asked for this handoff (2026-10-28).

### Decisions the user made this session

- **Voice transport: UDP** (option A). "if others use udp then i dont know why we would use webrtc". Recorded in `docs/fingerprint.md` "Voice" and 96's Answer. Thin `Transport` seam, no WebRTC work planned; openh264 out, Media Foundation for H.264.
- **Parity reference is the design, not oxidcord.** `parity mockup` where a Midnight mockup exists (`parity list`), else `parity capture --apps kyoko`, checked by eye against the ticket's Discord screenshots. Never build or run oxidcord; `oxidcord-ref	arget` is deleted (oxidcord-ref stays as source to port voice/video from). rules.md and common.md are updated.
- **Disk:** the user is short on storage. Clean an idle lane's `target` right after its merge (lane B's was wiped after 97: 27 GB -> 99 GB free). Ask before touching the 28 GB sccache cache (`%LOCALAPPDATA%\Mozilla\sccache`) or the user's roboco folders (in use by their Roboco workers). Finished worker chats get archived (73 archived this session).
- 75 crash: the user thinks it was a real crash, probably picking an option with the keyboard while typing; doesn't remember the time. Noted in 75. Next real run captures stderr.

### Merged this session

81 server profiles, 92 profiles like Discord (popout menu, DM column, full profile, themes; `.tooltip()` must be last in a zgx chain), 87 GIFs and stickers animate (streamed animations, Lottie renderer), 96 voice build trial (UDP decided), 95 channel settings, 97 voice connection core (`crates/voice`). Answers written for each.

### State now

- **Lane A** `kyoko-lane-a` at `e0f05f0` (84 plugin permission dialog scrim and scroll fix, worker `542c2b19`, done, all checks green per worker except the theme_links flake). Not yet verified by the orchestrator, not merged. Rebase onto main first (main has 97 since).
- **Lane B** `kyoko-lane-b`: **98 running** (worker `584d901b`, join from sidebar, voice panel, mute/deafen; base `c4ad329`, cold target). Its completion notice goes to `8e102e63`: poll it with `get_chat`/`read_chat`, or send it a short note so its next turn reports to you.
- **Lane C** `kyoko-lane-c` at `54d0e3f` (82 image grid, worker `fb0181ca`, done). Orchestrator verification was running in `8e102e63`'s background shell: fmt, clippy, build green; `test --workspace`, xtask test, headless-run, deny, luau and the kyoko-only captures (`timeline-attachments,timeline-image-grid,-three,-four,-ten`) were still running, then lane A's full verify and captures (`settings-plugins,settings-plugin-permissions`, plus `parity mockup --screens settings-plugins`). Logs: `<lane>/target/orch-verify.log` (one `=== exit` per step, `=== done`, then `for parity`). If cut off, rerun the missing steps; `.orchestrator/verify-rest.sh <lane>` runs from the test step on. Then rebase onto main, merge, Answer, clean the lane's target, archive the worker chat.
- **Known flake (new):** `theme_links_are_fetched_over_https_only_without_any_discord_credentials` fails when another process probes new localhost ports (stray `GET /` with only Host and Connection; probably the user's Roboco test runs). Rerun it. A test fix would be to ignore requests it didn't ask for; worth a small ticket.
- sccache sometimes drops a compile ("connection forcibly closed"): rerun the step.
- The engine restarted twice this session and cut every worker's turn: after a restart, check each worker and send a "resume" message.

### What's left (24 not resolved in v1; Midnight all resolved)

- Done, waiting for verify and merge: 82 image grid, 84 plugin dialog.
- Running: 98 voice join.
- To build: voice 99 DAVE (decide RUSTSEC-2026-0387 first: accept or a BoringSSL openmls provider; spec-level, ask the user), 100 voice/video settings, 101 call view and speaking, 103 video receive, 104 camera, 105 screen capture and Go Live picker, 106 Go Live viewing, 107 Go Live streaming; and 83 rail hover and voice indicator, 85 hover colours audit, 89 media viewer (use `bridge::mosaic_images`), 90 new logo, 91 inline video (replace the mp4 stand-in in `bridge::gifv_request`), 93 fetch concurrency (`timeline-image-grid-ten` is a load case), 94 voice row hover buttons (after 98).
- Waiting: 74 part 2b (Chrome 157 bump, 2026-11-03), 75 crash (needs a real run with stderr), 102 the user's own real call (`ready-for-human`), 108 DM calls (`needs-triage`). Skipped by the user: 39, 58.
- Suggested order: lane A 83, 85, 90, then 94 after 98; lane C 89, 93, 91; lane B 99 (after asking about the advisory), 101, 100, then video.

## Latest: tenth orchestrator hands off (roboco chat `223bd24d`, "kyoko orchestrator 10", same machine, main `6b6b73e` + this note)

Everything below still applies unless this section says otherwise. The user asked for this handoff (2026-10-28) and wants the new orchestrator to ask them the open questions.

### Ask the user first (in plain words, short)

1. **Voice transport** (plan `nguyenvu/.research/voice-plan/plan.md` section 2, tickets 96–108). The browser does voice over WebRTC; oxidcord, the desktop app, discordo and endcord use plain UDP, which is a visible mismatch for a client that presents itself as Chrome (whether Discord acts on it is unknown). Options: (1) UDP now, everything above the transport built once behind a `Transport` seam, decide WebRTC later from a localhost capture of Chrome's offer (recommended); (2) WebRTC only (faithful, weeks later; needs a str0m crypto backend on BoringSSL); (3) UDP only. Also: openh264 left out (Cisco's patent cover only applies to its binary), Windows Media Foundation for H.264, unless the user objects. The user's wishes: clicking a voice channel joins it, real talk/listen, camera, screen share, watching and starting Go Live.
2. **Slash-command crash (75, `needs-info`)**: not reproduced on mock (mouse, keyboard, typed command, em dash, autocomplete, the user's 7 plugins, ~1,500 random steps); no Windows crash record; the crashing run had no stderr capture. Ask: did they pick the option by clicking or by keyboard, were they typing a value; and did they close kyoko at 20:45 on 2026-10-27 or did it vanish (run 3's log just stops, no panic on stderr).

### State

- Real-account testing (user's go-ahead, 2026-10-27; throwaway token `KYOKO_TOKEN`, user drives the app, look-only rules for us). Launch recipe (PowerShell): set `$env:KYOKO_TOKEN` from the user-level variable, `RUST_BACKTRACE=1`, `Start-Process kyoko\target\debug\kyoko.exe -ArgumentList '--log','info,gateway=debug'` with stdout/stderr redirected to `nguyenvu\.orchestrator\real-run\stdout-N.txt`/`stderr-N.txt`, then remove the env var from the shell. Rebuild the main checkout first (`cargo build -p kyoko` with the BoringSSL prebuilt env vars, ~8 min cold). Check logs only with `grep -o` for key names, never print values. The gateway never disconnected in the user's run. Screenshots from the user: `nguyenvu\.orchestrator\real-run\screens\` (outside the repo; real people's messages). The user will test again once the current batch lands.
- Resolved this session after the TLS work: 75 (partly: caret gap, options at once, popovers float; crash open as `needs-info`), 76 (false "Reconnecting", sidebar reflow, isolated loading), 77 (profiles load on open), 79 (emoji picker), 80 (server opens its channel), 86 (caret blink, cursors), 88 (toasts). 78 is `wontfix`, superseded by the voice tickets. 74 part 2b (Chrome 157 bump) waits for 2026-11-03.
- **Running** (completion notices go to `223bd24d`, not to you: poll with `list_chats` parent `223bd24d` and `read_chat <id> limit 1 include_tools false`, or send each a short note so its next turn reports to you):
  - 81 server profiles, animated avatars and banners: lane A, `bdca7cbb`, base `d9ca670`. Next on lane A: 92 (DM panel, full profile, popout menu; blocked by 81).
  - 96 voice build trial: lane B, `6197a52b`, base `a976864`. Writes A/B/C "decision pending" in `docs/fingerprint.md` ("Voice"); tell it the user's answer if it comes first. Then 97 onward per the blocked-by lines.
  - 87 GIFs and animated stickers: lane C, `9dc9e7f0`, base `6b6b73e`.
- **Queue** (open, unblocked): 94 voice row hover buttons, 95 channel settings, 82 image grid, 89 image viewer, 83 rail hover and voice indicator, 84 plugin dialog and scroll, 85 hover colours audit, 93 image fetch concurrency, 90 new logo (user's image URL in the ticket; credit the artist), 91 inline video (start from oxidcord's Media Foundation decoder; share it with the voice video tickets). 102 is the user's own real call (`ready-for-human`), 108 DM calls `needs-triage`.
- Worker brief: `nguyenvu\.orchestrator\common.md` (point every prompt at it plus the ticket, Answers, lane, base, port). All lanes use the shared BoringSSL prebuilt env vars (`BORING_BSSL_PATH`, `BORING_BSSL_ASSUME_PATCHED=1`) in every cargo shell, the orchestrator's verify runs included.

### Gotchas this session

- Three lanes building at once on 32 GB ran out of memory twice: run your verifications one after another (a Bash `until grep ... orch-verify.log` chain), never in parallel.
- `verify.sh`'s luau line now greps `luau0-src` (a "Blocking waiting for file lock" line used to hide it).
- Workers often end their turn while checks run, and "finished" notices also fire when a queued "main moved" note starts a new turn: read one message further back (`offset`) for the real report. If a worker stopped mid-check, verify yourself after checking for stray lane processes.
- Rebase conflicts were all unions (`crates/state/src/state.rs` fields); after a conflicted rebase rerun the full verify.
- zgx lessons recorded in Answers: `ui.each` grows even with `grow = 0`; keyed `ui.each` rebuilds rows on new allocations (use `StableRows`); zgx cancels a component's tasks on rebuild; a `menu` attached to an element with handlers swallows left clicks; cursor lookup doesn't skip `pointer_events = "none"` (any empty full-window layer hides cursors).

## Latest: tenth orchestrator (roboco chat `223bd24d`, "kyoko orchestrator 10", same machine)

Everything below still applies unless this section says otherwise.

- Resolved this session: v1 70+71 (REST and gateway on wreq), 72 (QR socket on wreq), 73 (media and theme links on wreq; reqwest, rustls and aws-lc out of the shipped exe, release exe 78,870,528 bytes). 74 part 1 merged (capture of Chrome 156's API/image/websocket header order, `IMAGE_ACCEPT` with `image/jxl`, the "Bumping Chrome" runbook in `docs/fingerprint.md`); read 74's "Progress" section.
- AGENTS.md has the user-approved wording (merged with 70).
- **First real-account run (2026-10-27, user's go-ahead, throwaway token, user drove the app).** kyoko runs from the main checkout's `target\debug\kyoko.exe` with `--log info,gateway=debug`, output in `nguyenvu\.orchestrator\real-run\` (no token in it: checked; only READY key names match "token"). The gateway never disconnected while the user browsed. The user's findings became tickets 75–92; screenshots in `.orchestrator\real-run\screens\` (outside the repo, real people's messages). Running: 75 slash-command crash + popover (lane B, `98d16829`), 76 false "Reconnecting" on channel open (lane A, `b16080dc`), 78 voice join opens DM call (lane C, `08c2d9fd`). Shared worker brief: `.orchestrator\common.md`. Queue after them: 79 picker (click, hover, speed), 88 toasts, 80 server opens its channel, 86 caret blink and cursors, 77 → 81 → 92 profiles (one lane, in order), 87 GIFs/stickers, 82 image grid, 89 image viewer, 83 rail hover, 84 plugin dialog, 85 hover colours audit, 90 new logo, 91 inline video (big). The user will test again once these land.
- Left: **74 part 2b**: the Chrome 157 bump, after 2026-11-03 once Chrome 157 is installed locally, following the runbook in `docs/fingerprint.md` (see 74's part 2a notes). The user said yes to both header decisions (2026-10-27); part 2a implemented them on Chrome 156 (`dd3b3d0`).
- No workers running; lanes A, B, C idle on main. All lanes should use the shared BoringSSL prebuilt from now on: every shell that runs cargo in a lane (worker or orchestrator, including verify.sh runs) sets `BORING_BSSL_PATH='C:\Users\HP MEDIA\Desktop\nguyenvu\.cache\boringssl-129887582a53'` and `BORING_BSSL_ASSUME_PATCHED=1`. Lane A's and lane C's targets were deleted (cold, cheap with sccache plus the prebuilt). Disk ~185 GB free.
- Verification addition for network tickets: after verify.sh, `cargo xtask tls-capture --client wreq --h2 --compare chrome-<major>` must show 0 and 0 differences.
- Gotchas: workers ended their turn while checks ran three times this session; resend "continue, poll within the turn". In Python heredocs a `\t` in a Windows path became a tab once; use forward slashes in ticket text.

## Latest: ninth orchestrator hands off (roboco chat `2be6a038`, "kyoko orchestrator 9", same machine)

Everything below still applies unless this section says otherwise. The user went to sleep (2026-10-27) after saying "ok just do ur thing, handoff to new orchestrator chat if u need": keep working through the list without waiting for them, and stop only for things that need them (below).

### State

- Resolved this session: v1 68 (login card leads with "Sign in on discord.com" and the QR code; email behind "Use email and password instead") and v1 69 (TLS build trial: **go**, wreq's Chrome154 profile matches local Chrome 156 with 0 differences on https and wss). New tickets 68–74; 70–74 are the TLS plan's T2–T6. Read 68's and 69's Answers.
- libclang: the VS Clang component was 2.2 GB download / 5.7 GB installed, so I asked; the user chose the official **LLVM 23.1.4** (winget `LLVM.LLVM`, `C:\Program Files\LLVM`). I added `CMAKE` (VS 2022 Build Tools cmake) and `CMAKE_GENERATOR = "Visual Studio 17 2022"` to the user-level `~/.cargo/config.toml` `[env]`, so every shell and lane builds BoringSSL the same way (a different value reruns the ~4 min C build). The repo's `.cargo/config.toml` sets `HOST_CMAKE_TOOLCHAIN_FILE` to `.cargo/boringssl-no-asm.cmake` (no NASM). Shared BoringSSL prebuilt for btls rev `1298875`: `nguyenvu\.cache\boringssl-129887582a53` (`BORING_BSSL_PATH` + `BORING_BSSL_ASSUME_PATCHED=1`, see `docs/fingerprint.md`); lanes B and C haven't built BoringSSL yet (first build ~6.4 min dev).
- In flight: **v1 70+71 (REST and gateway on wreq), lane A, worker `53d90ff9`** ("kyoko 70+71 REST and gateway on wreq (lane A)"), started on `40a7b66`, two commits expected (70 then 71). Its completion notice goes to `2be6a038`, not to you: poll it with `list_chats` (parent `2be6a038`) and `read_chat 53d90ff9 limit 1 include_tools false`. When it reports: verify in lane A (`verify.sh` plus `cargo xtask tls-capture --client wreq --h2 --compare chrome-156`, which must stay 0 and 0), check the AGENTS.md wording below, merge, write both Answers.
- **AGENTS.md rule wording, approved by the user** (shown to them 2026-10-27; they answered "ok just do ur thing"): "Present the identify properties, REST and gateway headers, and the TLS and HTTP/2 handshake of the Discord web client in Chrome, nothing more; never automate user actions, bypass captchas, MFA, rate limits or client verification." plus `net` in the headless list. If the worker's wording differs, use this one.
- `deny.toml` ignores RUSTSEC-2026-0391/-0398/-0403 (zstd 0.13 / zstd-safe 7, only from wreq-util's Firefox-only cert compressor). Told to the user as their call; no answer yet. Keep unless they object.
- Next after 70+71: **72** (QR socket over `net::ws`, blocked by 71) and **73** (media and theme links on wreq; reqwest, rustls, aws-lc leave the shipped exe; blocked by 70) can run in parallel on two lanes (they touch different crates: `crates/login/src/qr` vs `crates/media`, `crates/session/src/theme_links.rs`; both touch `Cargo.toml`/`Cargo.lock`, expect a lockfile rebase). Then **74** (Chrome bump runbook, run once for 157 after its full stable 2026-11-03; it has a user-decision box about REST header order and websocket handshake headers: write the evidence and ask the user, don't decide). 69's Answer lists the notes each of them needs (`websocket_emulation()` clears ALPS; Chrome splits `cookie` on H2; image `Accept` gains `image/jxl` in 74).
- Lanes B and C idle on `40a7b66`. Disk 107 GB free; lane targets A ~37 GB, B/C ~28 GB.
- Not to do without the user: a CI run (pushing main runs CI anyway; that's fine as usual, but don't trigger extra runs or change workflows' secrets), real-Discord runs, releases, tags, upstream reports. v1 39 and 58 stay skipped.
- Status notes for the user: one short plain numbered list of what's left, no ticket jargon. Left now: (1) REST and live connection on Chrome's handshake (running), (2) QR login connection, (3) images/media and dropping the old network libraries, (4) the Chrome bump procedure, run for 157.

## Latest: eighth orchestrator hands off (roboco chat `a5bca87c`, "kyoko orchestrator 8", same machine)

Everything below still applies unless this section says otherwise.

### State

- Resolved this session: Midnight 14, v1 54, 49, 56 (merged, answered). Every ticket in both trackers is resolved except v1 39 translations and 58 packaging (`ready-for-human`, skipped by the user; don't start them). All three lanes are idle and fast-forwarded to main; no workers running. Disk ~198 GB free; lane targets ~22–26 GB.
- 49's crown rule hides the stock owner crown above 1000 members (kyoko's assumption, Discord documents none). Asked the user, no answer; keep it unless they say otherwise.
- `rules.md` flake list gained `kept::a_deleted_message_stays_in_place_flagged_and_survives_a_restart` and `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one`.
- Gotchas seen: completion notices for chats started by the previous orchestrator went to it, not me, and "finished" notices sometimes arrived while the worker was still running (check `list_chats` status). A worker ended its turn with tests still running in the background; nudge with "keep going and poll within this turn". `KYOKO_CAPTURE_PLUGINS=<id>` must be set by you when verifying kyoko-only plugin captures, or the plugin is absent.

### What the user approved (2026-10-27): do these next

The user asked about ban risk. Research (read-only, no Discord contact) compared kyoko with other clients; clones are in `nguyenvu/.research/` (`tls-check/` has discordo, discord.py-self, acheron, discord-client-rs and others). Findings: kyoko's headers and identify are more consistent than the TUIs', but its TLS/HTTP-2 handshake is a Rust program's (reqwest+rustls, tungstenite) while it claims Chrome. Clients that had captcha or "account disabled" trouble copied Chrome's handshake: discordo (bogdanfinn/tls-client, PR #824), discord.py-self (curl_cffi; changelog "Fix TLS fingerprinting issues causing unnecessary CAPTCHA challenges and blocked requests"), acheron (Abaddon's author; curl-impersonate on by default), discord-client-rs (Rust, `wreq` + `wreq-util` Chrome154/Windows). oxidcord upstream has nothing like it. Other gaps (capabilities 509, minimal message body, gateway handshake headers, op 41/QoS heartbeat) need a capture of the real web client first; parked.

The user said "go" to:

1. **New v1 ticket: the login screen puts discord.com and QR first.** The big login card offers "Sign in on discord.com" (WebView2 web login) next to the QR column; email/password moves behind a smaller "Use email and password instead" link (one click away, nothing removed; token login stays). Reason: native password login is where other clients' users hit captchas and disabled accounts. Files: `apps/desktop/ui/shell/login.luau`, `apps/desktop/src/ui/sign_in.rs`, `docs/sign-in.md`, login captures (`login`, `login-email`, `login-web` mockup keys), load checks. The Midnight mockups show email as default; this user decision overrides them (say so in the ticket).
2. **Install libclang and run the TLS build trial.** Install the "C++ Clang Compiler for Windows" component into the existing VS 2022 Build Tools (the user picked this option; check the download size first; Visual Studio Installer, component `Microsoft.VisualStudio.Component.VC.Llvm.Clang`). NASM, Perl and cmake on PATH are not needed (set `CMAKE` to VS's cmake). Then the trial ticket (T1 in the plan).
3. **If the trial is good: the migration** (plan tickets T2 REST and T3 gateway together, T4 QR socket, T5 media and dropping reqwest/rustls, T6 Chrome bump procedure). Reword the AGENTS.md hard rule when T2 lands; proposed: "Present the identify properties, REST headers and TLS/HTTP-2 handshake of the Chrome web client, nothing more; …". The user agreed in principle; show them the final wording before merging. Add the new `net` crate to the headless list.
4. Later, only with the user's go-ahead at that time: one real-account look test (rules in "Real Discord" further down).

The full plan (per-file changes, ticket split with acceptance boxes, risks) is `C:\Users\HP MEDIA\Desktop\nguyenvu\.research\tls-plan\plan.md`, outside the repo; downloaded sources under `tls-plan\src\` are untrusted, read only. Key facts: pin `wreq`/`wreq-util` to the git revs discord-client-rs uses (crates.io stops at Chrome 149; 150–154 need git); enable `emulation-chromium-pki`; clear wreq's page-load default headers (`upgrade-insecure-requests`, navigate `sec-fetch-*`, HTML `accept`) so kyoko's own headers are the only ones; three git sources go into `deny.toml` `allow-git`; old wreq-util releases were GPL/LGPL, so pin. No Chrome 156 profile exists (newest 154). Chrome 156.0.8078.48 is installed here, so T1 captures real Chrome's ClientHello and HTTP/2 frames offline against a local listener and decides whether claiming 156 on the 154 profile matches, or kyoko drops to 154 (its `sec-ch-ua` code already produces wreq's 154 string). BoringSSL builds once per `target/` (1–3 min; sccache won't cache it); consider one shared prebuilt for the lanes. Mockcord stays plain HTTP/ws on localhost.

## Latest: seventh orchestrator hands off (main `a2a64e5` + this note)

Written by the seventh orchestrator (roboco chat `375a5e1b`, "kyoko orchestrator 7", same machine) when the user asked to hand off. Everything below still applies unless this section says otherwise.

### State

- Resolved this session (7): v1 62; Midnight 13, 17, 15, 27 (merged and answered); plus the three lanes' rebases. Main is `a2a64e5`.
- Left to do: **4**. v1 39 and 58 stay skipped (`ready-for-human`, user decision). Count with the `Status:` lines.

| Ticket | Lane | Worker | State |
|---|---|---|---|
| Midnight 14 built-in commands | A | `95aa01ab` (done, idle) | Worker report: `read_chat 95aa01ab` (the long message before the last). I rebased it onto `a2a64e5` myself (head `427d5d1`; union conflicts in `composer/mod.rs` mod/use/struct fields, `bridge/src/lib.rs` image request imports, `tests/bridge/main.rs`, `docs/testing.md`, `docs/parity.md` (the unmapped-mockups line now reads "Every mockup page now has a kyoko screen."), `MOCKUPS`). Verification was running in my shell when I handed off: `kyoko-lane-a/target/orch-verify.log`, done when it has `=== exit N for parity` (screens command-picker-built-in, built-in-gif, built-in-sticker, built-in-thread, apps-launcher, command-picker on port 8494). If it was cut off (fewer exit lines, no cargo process for lane A), rerun `verify.sh` plus that parity line. Worker's parity: 12.6 / 18.9 / 16.2 / 13.3%. Then merge, write the Answer (protocol facts: frecency ids shrug -1 … sticker -17, schedule -19; `/schedule` left out, experiment-gated `/users/@me/scheduled-messages`; moderation built-ins open kyoko's dialogs instead of running at once; `/shrug` single backslash fixed; deviations in the report), claim 56 and launch it on lane A. |
| v1 49 PlatformIndicators + ForceOwnerCrown | B | `45f92384` (running) | Started on `7e7ddd0`; told main moved to `a2a64e5` (inbox) and that lane C's 54 overlaps in `docs/plugins.md`, `bundled.rs`, `crates/plugins/src/*`. Prompt points it at v1 52's Rust-drawn decoration pattern and the reserved `user.decoration`/`member.decoration` slots; kyoko-only captures with `KYOKO_CAPTURE_PLUGINS`. |
| v1 54 ReadAllNotificationsButton | C | `7566b982` (running) | Started on `a2a64e5`; uses the existing `rail.buttons` slot and `Command::MarkAllRead` (paced bulk ack), reuses M27's `unread_groups()`. |
| v1 56 GameActivityToggle | — | not started | Next on lane A after M14 merges. Uses `panel.buttons` (v1 47) and can reuse M19's `show_current_game` setting. |

- **Completion notices for 49 and 54 come to `375a5e1b`, not to you.** Poll them with `get_chat`/`read_chat`, or send each one short message so its next turn reports to you.
- Disk: 215 GB free; lane targets were 21–25 GB when last measured.

### Things to know (new this session)

- Union-conflict pattern on rebases (the same every time): `capture.rs` `Screen` variants and shot enums, `ui.rs` `layered(...)` list and capture arms, `ui/root.rs` `LAYERS` (bump the array length when both sides add one; it is 15 now), `timeline/icons.rs`, `docs/parity.md` rows and the unmapped list (apply both sides' removals), `MOCKUPS`, mockcord showcase sections, `bridge/src/lib.rs` re-exports, `session/command.rs`, `state/event.rs` imports. A small Python resolver (keep both sides; intersect the unmapped list) handled every one; re-verify after.
- The parity tool refuses a screen with no mockup (`timeline-replies has no mockup yet`): only pass `MOCKUPS` keys to `parity mockup`.
- `verify.sh` writes `=== done`, not a parity line; append the parity step yourself (`echo "=== exit $? for parity"`).
- A session restart kills background verifications; check the log for one exit line per step and rerun.
- New load flakes seen by workers (each passed alone): `kept::a_deleted_message_stays_in_place_flagged_and_survives_a_restart`, `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one`. Add them to rules.md's flake list.
- `get_chat` on a busy worker returns its whole tool list (very long); prefer `list_chats` with `parent` for status and `read_chat` with `limit: 1, include_tools: false` for reports.
- The user asked (2026-10-27) whether kyoko has been tried on a real account: no, everything so far is mock only (see "Real Discord" below). Ask before any real run.


## Latest: sixth orchestrator hands off (main `2bbf0fd` + this note)

Written by the sixth orchestrator (roboco chat `7d781d9c`, "kyoko orchestrator 6", same machine) when the user asked to hand off at about 44% context. Everything below still applies unless this section says otherwise.

### State

- Resolved this session (13): v1 63, 67 (new: pacing tests under load), 66; Midnight 20, 09, 22, 19, 10, 18, 23, 11, 25, 12. Added: Midnight 27 (inbox, user said yes 2026-10-27), v1 67.
- Left: **9**. Done, verification running (see below): Midnight 13, Midnight 17, v1 62. Not started: Midnight 27 inbox, Midnight 14 built-in commands, Midnight 15 apps launcher, v1 49 PlatformIndicators/ForceOwnerCrown, v1 54 ReadAllNotificationsButton, v1 56 GameActivityToggle. **Skipped by the user (2026-10-27), status `ready-for-human`, don't pick up: v1 39 translations, v1 58 packaging.** Count with the `Status:` lines.
- All three workers are finished and idle; nothing reports to you. Their reports are the source for the Answers (`read_chat <id>`, the long assistant message):

| Ticket | Lane | Worker | Branch head (rebased on `2bbf0fd`) | Verification |
|---|---|---|---|---|
| Midnight 17 polls | B | `6437069f` | `4d768d0`, `06bf81e` | Fully verified once before (poll-create 8.2%, poll-in-messages 13.7%, -ended 13.4%); its pacing test then moved to `assert_released_apart`. Re-verification running now. |
| Midnight 13 components and modals | A | `ed0822e7` | `d8eb152` | Running now. Worker: all green; parity message-components 15.8%, -multi-select 15.9%, -v2 23.7% (mock gallery colours), app-modal 10.8%, app-modal-error 10.7%; protocol facts from the public web bundle (UA `kyoko-dev`, no identity). |
| v1 62 moderation follow-ups | C | `cc8b4eb6` | `845287f` | Running now. Worker: all green; kyoko-only capture `moderation-roles-submenu`; submenus flip left via an `rtl` trigger (zgx places submenus against the monitor, not the window); held roles use `kit::checkbox`; a deleted open channel moves to `bridge::replacement_channel` (server's first visible text channel; not checked against the live client). |

- The three verifications were started in this session's background shell. Their logs are `<lane>/target/orch-verify.log`; done when it has `=== exit N for parity`. If this chat is gone they may have been cut off: check for one `=== exit` line per step, check for stray processes of that lane, rerun what's missing. Then merge in any order: rebase onto main (expect union conflicts in `capture.rs`, `docs/parity.md`, `MOCKUPS`, `ui.rs`), recheck fmt/clippy/build/`xtask test`/parity after a conflicted rebase, `merge --ff-only`, push, Answer, launch the next ticket on the freed lane.
- Suggested next: Midnight 27 inbox (Rust popout over the slot, Mentions fetched on tab open, Unreads from read state), Midnight 14 (built-ins; M10/M12 Answers say what to fill in), Midnight 15 (apps launcher; reuse `CommandPicker::jump`, M13's select popover), then v1 49/54/56 (plugin slots from 45/47; 56's toggle can reuse M19's `show_current_game` setting).

### Decisions the user made this session

- One design, Midnight. Two trackers are just v1 (features) and Midnight (redesign); the user found the split confusing, so **talk about the work as one plain numbered list, not "v1 vs Midnight"**.
- Build the inbox (Midnight 27). Skip translations (39) and packaging (58) for now.
- Earlier ones (from orchestrator 5's follow-up): no GIF `provider` (switch to `klipy` if search fails), web login uses WebView2's own user agent (done, 66), no test sign-in for web login.

### Things to know (new this session)

- **Completion notices can be missed.** Three finished workers sat idle for 3–4 hours while I talked with the user. Check every worker with `get_chat` whenever you answer the user, not only on notices.
- Workers often end their turn while their checks run in the background (10-minute foreground limit). Prompts now say: run long steps in the background and poll within the same turn. Still verify every lane yourself; don't rely on the worker's last run.
- Queued "main moved" messages make idle workers start a new turn per message. Send one per merge with `mode: queue` only to workers still building; once a worker has reported, steer it with "stale, don't rerun, reply ok" or just verify yourself.
- `cargo xtask test` stops at the first failing suite, so a flake there skips the desktop and Luau load checks: rerun `xtask test` alone.
- Pacing tests must use `running.assert_released_apart` (v1 67); gaps between mockcord arrivals are banned. rules.md's flake list was trimmed accordingly.
- Toasts live in a zgx `.layer()` portal so they show above open Luau modals (M23 found them hidden under settings). Rust dialogs and menus are zgx portals and already go on top.
- Parity screen names: use the `MOCKUPS` keys (`login-web`, not `login`). The ticket cell in `docs/parity.md` must be digits only or xtask drops the row.
- Python edits of ticket files: a `\\0` in a heredoc string became a NUL byte once (Midnight 10's Answer; fixed). Write literal backslashes as `bytes([92,48])` or check with `grep -c` afterwards.
- One `git push` was rejected by GitHub once and passed on retry.
- Open real-Discord confirmations added: M11 file option upload body, M17 whether polls also send `content`/`tts`/`flags`, v1 63 exact dialog wording, v1 62 which channel the web client opens after deleting the open one. All need the user's account; don't run them without asking.
- Disk: 212 GB free; lane targets 22–27 GB.

## Latest: fifth orchestrator hands off (main `07233b8` + this note)

Written by the fifth orchestrator (roboco chat `051e1203`, "kyoko orchestrator 5", same machine) when the user asked to hand off before implementing Midnight 19. Everything below still applies unless this section says otherwise; the "Decisions during the fifth orchestrator" list right under this section is part of it.

### State

- Resolved this session: v1 53, 42, 52, 43, 55, 44, 65 (new: stale timeline node panic); Midnight 03, 26 (new: drop the member search field), 05, 24, 07, 08, 21, 06, 16. Added: Midnight 25 (Discord theme variables on the Midnight tokens), 26, v1 65; `PRODUCT.md` at the repo root (for `/impeccable` design work).
- Open v1: 39, 49, 54, 56, 58, 62 (plus 63 in flight). Open Midnight: 10–15, 17, 18, 19, 22, 23, 25 (plus 09 and 20 in flight). Count with the `Status:` lines. Midnight 08 is resolved except its box 2 (the live GIF-provider check, waiting on the user).
- In flight (completion notices go to `051e1203`, not to you: send each a short message so its next turn reports to you, or poll with `get_chat`/`read_chat`):

| Ticket | Lane | Worker | State |
|---|---|---|---|
| Midnight 09 emoji autocomplete | A | `84567cc5` | Running on `7f5df55`: shared lock rule in the bridge, a mockcord `--autocomplete-emoji` flag, the new container in `composer.luau`, capture and `MOCKUPS` mapping; it was at the parity capture. |
| v1 63 friend confirmations | B | `057760c9` | **Done, reported, not verified.** Commit `ffe1eb2` on `df38291`: `friend_dialog.rs` root layer (added the 61 way), `Link::ask_remove_friend`/`ask_block`, actions `friends.ask_remove`/`ask_block` (`friends.block` removed), the profile's Block sends `profile::Request::Block`, an in-process test (Cancel sends 0 requests, Confirm exactly 1), kyoko-only captures `friend-remove-dialog` and `friend-block-dialog`. Wording only partly confirmed from public sources ("permanently" and the Block explanation need a live check). To do: rebase onto main, verify in lane B, merge, write the Answer from its report (`read_chat 057760c9`). |
| Midnight 20 profiles and status | C | `5cf90259` | **Done, reported, not verified.** Commit `7bb36eb` on `df38291`: popout, full profile (900×640), the status menu as Discord's account popout with status and accounts submenus (v1 41's switcher still works), the custom status dialog; `ProfileCard` gained `GuildTag`, `ProfileConnection`, `guild_name`; 13 derived tokens; mockcord gives users whose id is a multiple of 3 a "RUST" tag; Log Out removed from the status menu (it lives in settings only). Parity: popout 20.1%, full-profile 27.7%, status-menu 18.2%, custom-status 13.1%. Load flake it saw: `favourites::a_favourited_emoji_is_read_then_written_to_the_frecency_proto_and_unfavouriting_removes_it` (passed alone; add it to rules.md's list). To do: rebase, verify in lane C, merge, Answer from its report. |

- Next, suggested: **Midnight 19 settings dialog** on the first free lane. The user just shaped it with `/impeccable shape`; the confirmed design brief is at the end of the ticket, follow it (Activity Privacy added, Voice & Video listed greyed, Billing/Nitro/Clips/Registered Games/Game Overlay left out, today's "managed in the web app" note pages kept, one "Kyoko" group, search finds pages and settings, production-ready). Then M10 command picker (after M09: anchor to the `composer` slot, option bar in `joined_bars`), M22 threads/forums/inbox/search, M23 server settings, M17 polls, M15 apps launcher, M18 voice message playback, M11–M14 command follow-ups, M25 theme mapping, v1 49/54/56 plugins, v1 62, 39 translations late, 58 last (releases need the user).

### Decisions the user made this session

- Design-first: where a Midnight mockup differs from a v1 behaviour, follow the mockup (M26 dropped the member search field). Don't ask about such cases.
- Live markdown in the composer: accept partial for now (zgx's textarea only draws range backgrounds; what zgx would need is in Midnight 16's Answer and `docs/ui.md`). No upstream report.
- PRODUCT.md (register product; users: the maintainer, then Discord users wanting a lighter native client; personality from the Midnight design; WCAG AA as now).
- Midnight 19's brief as above.
- Settled after the handoff (user, 2026-10-26, after comparing with oxidcord): Midnight 08 box 2 closed without a live check (no `provider`; switch to `klipy` if GIF search fails); web login gets WebView2's own user agent like oxidcord (new v1 ticket 66, small); no test sign-in for web login (the first real use is the check). Nothing is waiting on the user now.

### Things to know

- Verification: `C:\Users\HP MEDIA\Desktop\nguyenvu\.orchestrator\verify.sh <lane path>` runs fmt, clippy, `build --locked`, `test --workspace --locked`, `xtask test`, `headless-run --mock`, `deny`, `cargo tree -i luau0-src` into `<lane>/target/orch-verify.log` with `=== exit N for <cmd>` lines. Run it from Bash in the background, then grep `^=== (exit|done)|\.\.\. FAILED`; append a `cargo xtask parity mockup --port <lane port> --screens ...` for UI tickets. Check the log has one exit line per step (a stopped run keeps writing; see the gotcha below).
- Queue verifications one after another when two lanes finish together (a Bash `until grep -aq ...; do sleep 30; done` wait on the first log), and rebase a lane onto main before verifying if main moved under it. Conflicts so far were all list unions in `capture.rs`, `docs/parity.md`, `docs/ui.md`: keep both sides (for the unmapped-mockups list, apply both sides' removals).
- sccache dropouts ("failed to execute compile") still happen under load; rerun the step. New load flakes: `safety::a_rejected_token_stops_every_request_and_signs_out` (in rules.md) and M20's favourites test above.
- Disk: clean an idle lane whose target passes ~60 GB; lanes reach 55–76 GB after two or three tickets even with `CARGO_INCREMENTAL=0` (every prompt sets it). C: had ~290 GB free at this note; lane A was cleaned before M16, B before M21, C before M06.
- Prompts that worked: point at `rules.md`; ticket path; worktree, branch and base commit; `CARGO_BUILD_JOBS=3` and `CARGO_INCREMENTAL=0`; the lane port; the Answers it builds on with their key facts; the mockups and the `parity mockup` mapping; overlap warnings about the other two lanes; the done-means list. Send one "main moved to X (what landed, what it touches)" message per merge to each running worker.

## Decisions during the fifth orchestrator (chat `051e1203`)

- The Midnight design is the authority: where a mockup differs from a v1 behaviour, follow the mockup (user, 2026-10-26). M03's member search field is dropped (M26).
- Verification gotcha: stopping a background verification task (TaskStop) leaves its `verify.sh` child running, which then interleaves with the next run in the same `target/orch-verify.log`. Before rerunning in a lane, check for stray `cargo`/`rustc`/`verify.sh` processes of that lane, and read the log for one `=== exit` line per step.

## Earlier: fourth orchestrator hands off (main `f9ececd` + this note)

Written by the fourth orchestrator (roboco chat `bde24e85`, "kyoko orchestrator 4", same machine) when the user asked to hand off. Everything below still applies unless this section says otherwise.

### State

- Resolved this session: 34, 41, 47, 46, 51, 57, 48, 50, 64 (new: flaky headless display check), and Midnight M02, M04, M01. Resolved overall (v1): 01–38, 40, 41, 45–48, 50, 51, 57, 59–61, 64.
- Open v1: 39, 43, 44, 49, 52, 54, 55, 56, 58, 62, 63 (plus 42 and 53 in flight). Open Midnight: M05–M24 (plus M03 in flight). Count with the `Status:` lines in both `issues/` folders.
- In flight (completion notices go to `bde24e85`, not to you: send each a short message so its next turn reports to you, or poll with `get_chat`/`read_chat`):

| Ticket | Lane | Worker | State |
|---|---|---|---|
| M03 floating-panel shell | A | `44a487c1` | Started on `7df0010` (lane A target was cleaned first). Big restyle of every shell region; told to recommend whether Discord variable names in `CSS_MAPPING` should also land on the Midnight tokens. One queued "main moved to `f9ececd`" message. |
| 42 QR login | B | `e2a36be9` | Started on `f9ececd`. Look target is the Midnight login mockups (`login.html`, `login-qr-scanned.html`, `login-qr-expired.html`); ticket 42's first box was rewritten to say so. Remote-auth handshake in the session, mockcord plays it, captcha only shown, never solved. |
| 53 ExpressionCloner | C | `5e8ce6a8` | Done and reported (`ui:message.menu` slot with `[[menu]]` manifest items, `PluginHost::menu_gesture`, clone dialog `Dialog::Clone`, session-side upload validation, `CreateEmoji { image }` / `CreateSticker { file }`). Told to drop its commit `1daeb4f` (duplicate of 64's fix), rebase onto main, run the checks once and reply. Then verify in lane C (rebase, checks, look at `message-menu-clone` and `clone-dialog` captures), merge, write the Answer from its report. |

- Next, in suggested order. Lane A after M03: M05 timeline restyle (needs M03, M04 done), then M07 menus/dialogs/toasts, M16 composer. Lane B after 42: 43 web login, then 44 (both to the Midnight login mockups; 41's and 42's answers say what to reuse). Lane C after 53: v1 behaviour or Midnight work that doesn't touch the shell while M03 is in flight (55 hidden channels touches the sidebar, so after M03; 49/52 need Rust drawing per 47's answer; 54/56 use 47's slots and need M03's rail and user panel). Once M03 lands, M08/M09/M10 (expressions, autocomplete, commands) and M21 open up, and v1 62/63 take the Midnight look. 39 translations when few UI tickets are in flight. 58 last; releases and tags need the user.

### Decisions the user made this session

- Midnight effort (`.scratch/kyoko-midnight/`, spec `docs/design/midnight.md`, 101 mockups in `docs/design/mockups/`): foundation first (M01, M02, M04 done, M03 in flight), the third lane keeps doing v1 behaviour tickets.
- Look target: v1 UI tickets that haven't started match the Midnight mockup where one exists, checked with `cargo xtask parity mockup` (M02); tickets in flight when this was decided finished to oxidcord's look. Screens outside the shell (login) can start before M03.
- Nobody uses kyoko yet: no backward compatibility, migrations or shims (also in `rules.md`). M01's settings migration was removed because of this.

### Things to know

- Disk: on 2026-10-26 C: filled to 0 bytes; lane targets had grown to 90–124 GB each. Check `Get-PSDrive C` before launching a worker; `cargo clean` an idle lane whose `target` passes about 60 GB (sccache makes the rebuild cheap); never clean a lane while its worker builds. Lanes A and B were cleaned; lane C's target was about 90 GB and its worker builds with `CARGO_INCREMENTAL=0`; clean it between 53 and the next ticket.
- Verification recipe that worked: a background PowerShell loop over fmt, clippy, `build --locked`, `test --workspace --locked`, `xtask test`, `xtask headless-run --mock`, `deny check` (plus a capture) logging to `target/orch-verify.log` with `=== exit N for <cmd>` lines, then `Select-String` for `^=== exit|\.\.\. FAILED`.
- Known load flakes still seen: `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus`, `community::forwards_are_paced_sends_and_respect_locks`, `storage::turning_the_opt_in_on_and_off_moves_the_secrets`, `permissions::edited_plugin_files_reload_while_kyoko_runs`; each passed alone. The plugin "took too long to load" flake is fixed (46, 15 s test load deadline); 48's headless display flake is fixed (64).
- Queued "main moved" messages pile up: a worker answers each one after its turn and may re-run all checks for each. Prefer one message per worker per merge, and steer (`mode: steer`) with "these are stale, rebase once" when several are queued.
- Several tickets now add plugin hooks in the same files (`crates/plugins/src/{calls,hooks,host,limits,permission,vm,bundled}.rs`, `docs/plugins.md`, `tools/xtask/src/headless.rs`): the second to land conflicts. Send that rebase back to its worker.
- Shared `oxidcord-ref`: its tree equals `tools/parity/oxidcord-ref.patch` on main (34's `SETTLE`, 41's `account-switcher`, 47's `composer-silent-typing`); the exe was rebuilt with 34's and 41's steps. New Midnight screens compare with mockups, so oxidcord changes should now be rare.
- `KYOKO_CAPTURE_PLUGINS=<id,...>` (48) enables bundled plugins in a capture.
- A worker once put the user's email in a crates.io User-Agent (told to the user); `rules.md` now forbids the user's identity in any request.
- Open real-Discord confirmations added: 50 (when the web client first loads settings proto type 2), 53 (whether the web client sends `roles: []` on emoji create), M08 (GIF provider Klipy vs `provider=tenor`).

## Earlier: third orchestrator hands off (main `85cd454` + this note)

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
