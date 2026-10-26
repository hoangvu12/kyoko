# 21 — Home, DMs and friends

**What to build:** Restyle Home with Friends (Online, All, Pending, Blocked, Add Friend) and Active Now, DMs and group DMs (with the profile column) to the mockups.

**Blocked by:** 03; v1 24, 34

**Status:** resolved

**Mockups:** `docs/design/mockups/home-friends.html`, `docs/design/mockups/friends-all.html`, `docs/design/mockups/friends-pending.html`, `docs/design/mockups/friends-blocked.html`, `docs/design/mockups/friends-add-friend.html`, `docs/design/mockups/dm.html`, `docs/design/mockups/group-dm.html`

**Spec:** `docs/design/midnight.md`, Structure: floating panels

- [x] Matches the mockups

## Answer

Merged to main as `ded8305` (rebased over a `.scratch` commit). Docs: `docs/ui.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md` (Lucide `chat_plus`, `user_plus`, `panel_right`).

- Home: M03's `home_cover` is split into a friends header panel, a list panel and a 360-wide Active Now panel on a `bg_3` layer covering the page column (timeline slot and window controls behave as before). Nitro, Shop and Message Requests get a plain header and a placeholder panel.
- Friends: header tabs on `active`/`hover`, Pending count in an `accent_new` badge, Add Friend in `green_2` (`accent_5` while open), New Group DM and inbox; 40-high search; section titles as the mockups ("Online — 6", "All Friends", "Received", "Sent", "Blocked"); rows 62 high with `border` dividers, 32-point avatars with status dots, 36-point round buttons (Accept `green_2`, Ignore/Cancel/Unblock `red_2`). Add Friend: field with the send button inside, `accent_2` focus, result line `green_1`/`red_2`, "Other places to make friends" → `rail.discover`. Active Now (`bridge::active_card`): one card per friend with a real activity (name, activity with elapsed time, details, state); a custom status alone gets no card.
- DMs: header with the recipient's 24-point avatar and status (groups an `accent_4` circle), call, video, pins, Add Friends to DM, profile/member toggle, search, inbox. 1:1 profile column (340, `bridge::DmPanel` from `ProfileCard` without guild; `shell/dm_profile.luau` reads `dms.profile`): banner (image, else accent colour, else `bg_2`), 80-point avatar in a ring, name and handle, About Me, Member Since, Mutual Servers/Friends once cached, View Full Profile (`dms.full_profile` → `Link::open_full_profile`). Opening a 1:1 DM sends the existing `Command::LoadProfile` (no guild) once per open; the session's five-minute cache keeps it to one GET. Group member column (264, `dms.members`): "Members — N", everyone incl. you, by name, crown on the owner, activity line, rows slide 18. Both columns from 1200 wide while search is closed (`state.shell.dm_profile_visible`; two `ui.show` entries after the member list in `frame.luau`).
- Tokens: Midnight only, plus derived `friends_button_size`, `add_friend_input_height`, `dm_profile_banner_height`, `dm_profile_avatar_top`, `dm_profile_avatar_ring`, `dm_profile_content_top`. `capture.rs` untouched; Home captures wait for `Link::dm_panel_settled`.
- Tests: bridge `tests/bridge/dm_panel.rs` (`a_dm_shows_its_recipients_profile_and_the_mutuals_only_once_loaded`: three sends, one GET; `a_group_dm_lists_every_member_with_the_owner_marked`; `active_now_shows_a_friends_game_with_how_long_they_have_played`), load check `the_dm_columns_show_the_profile_and_the_group_members`. Checks re-run by the orchestrator in lane B: all green.
- Mockup parity (port 8496): friends-online 7.6%, friends-all 7.8%, friends-pending 6.9%, friends-blocked 6.4%, friends-add 7.8%, dm-sidebar 13.6% (the mockup's timeline starts with the DM welcome header), dm-group 8.4%.
- Deviations: no In Voice card / Join Voice (voice out of scope), no game art, elapsed time updates on the next presence change; DM profile has no clan tag, no banner gradient, name in `accent_1` not profile colours, avatar fully over the banner; DM search placeholder "Search"; Add Friends to DM and `dms.create_group` are stubs; toggle buttons use `text_1` active; the divider under a hovered friend row stays; the DM welcome header is not built.
- Found: an intermittent `zgx: contained a panic (stale scene node)` in `timeline/inline.rs:134` (`rich_node` from `inline::place`) when Home collapses the slot, ~3 in 20 captures: ticket 65.
- For later tickets: M20: profile theme colours, badges, clan tags and connections go in `shell/dm_profile.luau`; full profile and popout are still the Rust `profile/` layer. v1 63: the More menu items `friend_remove_<id>`/`friend_block_<id>` call `friends.remove`/`friends.block` directly; put the dialogs there with M07's `kit` and update `the_friends_page_reaches_every_relationship_action`. `friends.active` rows carry `details` and `state`.
