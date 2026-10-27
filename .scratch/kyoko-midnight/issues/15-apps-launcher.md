# 15 — Apps launcher

**What to build:** The composer's apps button opens the launcher popover, anchored like the expression picker (498 wide, bottom 8 above the composer, as tall as the chat allows): search across apps and commands, Recents, Apps in this Server with View More. Picking an app opens the command picker filtered to it. Discord's Promoted activities are left out.

**Blocked by:** 10

**Status:** resolved

**Mockups:** `docs/design/mockups/apps-launcher.html`

**Spec:** `docs/design/midnight.md`, Composer buttons

- [x] Matches the mockup
- [x] Search finds apps and their commands
- [x] No activities are offered

## Answer

Merged to main as `c9f4623..4d20e2d`. Docs: `docs/ui.md`, `docs/testing.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md`.

- Data (`crates/bridge/src/composer/apps.rs`): `apps_launcher` lists apps in command index order, only those with at least one slash command, with the index's `description` (`CommandApplication` gained it; fields from userdoccers `interactions/application-commands`). Recents are the apps of your frequently used commands (settings proto field 7), in that order. `search_apps` matches app and command names, prefix first, a leading `/` ignored. `CommandPicker::only_app` keeps one app's section; the composer gained `app_filter`, `open_app`, `pick_command` (the filter clears when the query stops being a command, a command is picked or the picker is dismissed).
- UI (`src/apps_launcher.rs`, root layer "apps launcher" in `layered(...)` and `LAYERS`; wiring in `ui/apps.rs`): 498 wide, right edge 35 in from the composer's right, bottom 8 above the composer, top 8 below the chat panel; search, Recents tiles, Apps in this Server, View More pages with a back arrow. Escape, an outside press, a channel switch and sign-out close it. The apps button sits after emoji and reuses `composer.picker` with tab `apps` (no new Luau state); the `+` menu gained Use Apps (main composer only). Picking an app opens the command picker filtered to it; picking a found command opens it at once.
- No activities: only apps with slash commands are listed, entry-point commands (type 4) never appear, Promoted isn't built.
- Mockcord: mockup descriptions; `--more-apps` (parity tool, this screen only) adds Warden, Lyra, Ticketeer, Countr, the activity-only Arcade, and seeds frecency so Recents read M R P S L W.
- Tests: bridge `apps.rs` (`search_finds_apps_and_their_commands`, `picking_an_app_opens_the_command_picker_filtered_to_it`, `the_launcher_lists_recent_apps_and_the_apps_in_this_server_but_no_activities`); in-process `ui::root::the_apps_button_opens_the_launcher_and_escape_or_a_press_outside_closes_it`. No new network call (reuses `LoadApplicationCommands`).
- Checks re-run by the orchestrator in lane B on `4d20e2d`: fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny all exit 0; luau0-src `0.22.0+luau740`. Mockup parity (port 8496, `pair.png` checked by eye): apps-launcher 12.4%, command-picker 11.7% (unchanged). The worker hit a new load flake, `kept::a_deleted_message_stays_in_place_flagged_and_survives_a_restart` (passed alone).
- Deviations: zgx draws a dark frame instead of the shadow (as M08); the hovered row isn't staged; the apps button has no active colour while open; Recents use command frecency (the web client may keep a separate per-app frecency, unverified); search matches names only, not descriptions; picking an app replaces the draft with `/`; app icons use a square CDN request, untested (no mock app has an icon); the thread composer's apps button does nothing.
- For later tickets: zgx `scroll(...)` without an explicit height comes out 240 high; give it the content height plus `min_h(0.)`. Built-ins aren't in the launcher's search.
