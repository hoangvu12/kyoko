# 30 — User panel, status and profiles

**What to build:** The user panel, the status and custom status controls, the profile popout and the full profile all match oxidcord.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] The user panel, popout and full profile match oxidcord in the parity captures
- [x] Changing status and custom status works
- [x] Presence and activities show on profiles

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `e1fbe6c..d43a1ca`. Docs: `docs/ui.md`, `docs/parity.md`.

- Session and state: status and custom status live in settings proto field 11 (`crates/protocol/src/settings_proto.rs`), written by a paced writer with rollback (`crates/session/src/task/status.rs`, only that field per PATCH) and sent as op 3 presence; own status in `crates/state/src/own_status.rs`. Profile fetches (`task/profiles.rs`) are cached per user and guild for 5 minutes and fail quietly. Custom status emoji is Unicode only (`SetCustomStatus`). 5 seam tests in `tests/session/status.rs`.
- Bridge `crates/bridge/src/profile.rs`: `ProfileCard` (user, member, cached profile, presence, custom status, activities, `dm_lock`), `PanelUser` (own status, custom status, gateway latency), `StatusChoice`. Bridge tests `the_panel_and_own_card_follow_a_status_and_custom_status_change`, `a_profile_card_shows_presence_activities_and_the_loaded_profile`.
- Window: user panel in Luau (`ui/shell/user_panel.luau`, shared value `user.panel`; avatar + status dot, name, custom status or username, mute/deafen/settings). The status menu, custom status dialog, profile popout and full profile are drawn in Rust by `apps/desktop/src/profile/` (`ProfileLayer`), above the timeline slot, covering the window only while open; click outside or Escape closes. The custom status emoji uses 28's reaction picker as a popover.
- One entry point: `Link::open_profile(user_id, at, anchor)` in `src/ui/profiles.rs`. Timeline avatar/author/mention clicks (28's `TimelineView::on_profile`) open the popout at the pointer, anchored top-left; member rows (29's `Link::open_member_profile`, `members.open_profile` now passes the pointer) anchor top-right, as oxidcord. Message opens the existing DM or sends `OpenDm` (held by the session's DM lock, `task/locks.rs`; the popout has no confirm step yet).
- 21's right-click Log Out is gone; Log Out is in the status menu (`login.sign_out`). The cog opens settings on My Account, Edit Profile opens settings on Profiles (37). Mute and deafen only toggle their icons (no voice).
- Verified in-process with `Ui::dispatch` (set Do Not Disturb, custom status "brb coffee" for 4 hours; member-row popout showing "PLAYING Hades II"). Luau load check gained a user-panel case. Checks re-run by the orchestrator in lane A.
- Parity (port 8494, 150% scale, 1920×1200): app-frame baseline 6.1%, user-panel 6.1%, status-menu 6.3%, custom-status 4.1%, profile-popout 7.4%, full-profile 4.4%. Remaining: avatar images, dimmer custom-status placeholder, popout a few points shorter. Beyond oxidcord: popout and full profile also show activities and custom status; bio is plain text (no markdown).
- For 41: Switch Accounts in the status menu sends `Request::SwitchAccounts`, logged by `Link::profile_request` in `src/ui/profiles.rs`; put oxidcord's accounts view inside `profile/status_menu.rs` (a measured column, a second view can swap in); `PanelUser` holds the current `User`.
- For 49: `ProfileCard.status` is a `DmStatus` from `presence.status`; `client_status` is not projected yet; add it to `ProfileCard`/`PanelUser` and draw it next to the badge in `profile::parts::avatar`.
- For other overlays: add a surface to `ProfileLayer` or a sibling layer in `ui.rs`'s root `overlay()` after `timeline_slot`; zgx absolute boxes don't stretch from `top`/`bottom` alone, size them from the window signal.
