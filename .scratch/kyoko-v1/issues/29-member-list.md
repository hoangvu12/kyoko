# 29 — Member list

**What to build:** The member list matches oxidcord. It is grouped by role and status, and loads more members as the user scrolls.

**Blocked by:** 20 (Theme, fonts, icons and app frame), 14 (Member list ranges and member-request scheduler)

**Status:** resolved

- [x] The member list matches oxidcord in the parity captures
- [x] Scrolling requests further ranges and fills them in
- [x] Presence and activity update live

## Comments

From the spike (04): one open real server produced 60–76 member-list updates in about 4 minutes; coalesce them in the bridge and keep them off the message list's update path.

## Answer

Merged to main as `884ed66..ca75446`. Docs: `docs/ui.md`, `docs/parity.md`.

- `bridge::MemberPanel` (`crates/bridge/src/member_list.rs`), host `apps/desktop/src/member_list.rs` and `src/ui/members.rs`. Rows are group headers or members from `State::member_list` groups corrected by loaded slots (capped at 100,000); only visible rows ±12 are published; unloaded rows are kyoko-designed placeholders (oxidcord has none). Member-list, guild-list and in-window presence changes only mark the panel dirty; its own task publishes at most every 150 ms (`MEMBER_PUBLISH_GAP`), separate from the timeline; `members.heights` republished only on layout change.
- Luau reports scroll offset and height; Rust computes visible rows and sends `SetMemberViewport` on change. Search filters loaded members at once, then `SearchMembers` after 350 ms if the guild isn't fully loaded.
- Luau `shell/member_list.luau` on zgx's `ui.list` with per-row `heights`: search field, "ONLINE — 39" headers, 42 pt rows (avatar + status dot from 24's components, role-coloured name, BOT tag, badge, activity line, offline at 50%). Hidden below 1200 px and in DMs; scrolls to the top when a channel opens.
- State fix: a SYNC for a range past the end no longer pads the list with never-filled rows.
- Tests: 4 bridge tests (`crates/bridge/tests/bridge/members.rs`; harness `start_with(MockConfig)`), incl. coalescing (24 updates → ~1 publish per 150 ms). Parity: member-list 7.0%, member-list-scrolled 6.3% (oxidcord steps exported to the patch); row positions line up; remaining is avatar art and text.
- Mockcord sends no list ops on presence changes, so offline members stay in the online group in mock runs.
- Rebase hazard found here: a merge cut an `end` out of `actions.luau` and every check still passed; ticket 60 adds a Luau load check. Until then, launch the app or run a capture after each rebase.
- For 30: member-row clicks call `Link::open_member_profile(user_id)` in `ui/members.rs` (Luau action `members.open_profile`; open guild via `self.members.panel().open()`), a logging stub. 36: no row context menu yet; add it in `member()` in `shell/member_list.luau`. 49: extend `MemberEntry` (owner from `GuildState::owner_id`) and the record in `apps/desktop/src/member_list.rs`; draw in `name_line()` beside the BOT tag.
