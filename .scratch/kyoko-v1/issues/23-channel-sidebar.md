# 23 — Channel sidebar

**What to build:** The channel sidebar matches oxidcord: categories, collapse state, unread and mention badges, and muted styling.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] The sidebar matches oxidcord in the parity captures
- [x] Selecting a channel opens it and subscribes to it
- [x] Collapse state persists

## Answer

Merged to main as `c2ee1ab..0daa14c`. Docs: `docs/ui.md`, `docs/rest.md` ("Collapsed categories"), `docs/parity.md`.

- Bridge `Sidebar` (`crates/bridge/src/sidebar.rs`): `SidebarSection`/`CategoryEntry`/`ChannelEntry`/`VoiceMember`/`GuildHeader` from `guild_channel_tree` + `guild_summary`; recomputes on the guild's `channel_lists`, `read_state`, `guild_list`, `user_settings` changes, publishes only on change. A collapsed category keeps the open channel, unread channels and occupied voice channels (as oxidcord). `ChannelKind::opens_timeline()` excludes Forum; `ChannelEntry::selectable()` is false for locked (not viewable) channels, shown with a lock as oxidcord does.
- Host `apps/desktop/src/sidebar.rs` (`SidebarView`): `sidebar.rows` (flat keyed list of category and channel records), `sidebar.header` (unread, muted, owner, `can_invite`, `can_manage_channels`, `can_open_settings`), `sidebar.guild_id`, `sidebar.guild_name`; `sidebar.channels` is gone. `SidebarView::reset()` runs on session start beside `RailView::reset()`. Voice avatars are 24 pt gallery circles, part of `Link::settled`. Luau `ui/shell/channel_sidebar.luau` (header, category, channel components; `header_menu`, `channel_menu`). Theme sizes `voice_row_height`, `voice_row_avatar`, `voice_badge_icon`.
- Selecting a text/announcement/voice/stage channel or thread sends `Command::SelectChannel` (subscribes) and opens the timeline; voice opens its text chat, never joins voice. Threads open in the timeline.
- Collapse: `Command::ToggleCategory` applies at once; session `task/collapse.rs` coalesces per guild (`settings_delay` 1 s) and sends `PATCH /users/@me/guilds/{g}/settings {"channel_overrides":{"<cat>":{"collapsed":bool}}}` through the mutation lane, at least `RestTuning::collapse_gap` (2 s) apart; a failure rolls back with a rejection; a newer pending toggle wins over a gateway echo; the echo lands in 13's versioned guild-settings container so collapse survives a restart. oxidcord only reads the field; kyoko writes it as the web client does. Unsent toggles at quit are lost.
- Menus are zgx menus below their anchor (`context_menu_below`), so they don't overlap the message area. Working actions: `sidebar.toggle_category`, `mark_channel_read`, `mark_guild_read`, `copy_channel_link`. Logging stubs: `invite`, `server_settings`, `create_channel`, `notification_settings`, `mute_guild`, `mute_channel`, `edit_channel`, `delete_channel`, `leave_guild`. Items have no icons; no mute-duration submenus; no Copy Server ID.
- Tests: 4 bridge tests (`crates/bridge/tests/bridge/sidebar.rs`, harness gained `restart()`), 4 session-seam tests (`tests/session/collapse.rs`). Parity (whole window): channel-sidebar 9.3%, channel-sidebar-collapsed 9.2%, server-rail 9.3%; sidebar region ~3.4–4%. The oxidcord reference sometimes fails a capture right after another; rerun the screen alone.
- For 24: reuse the region; when Home is selected (`sidebar.guild_id == ""`) swap `channel_sidebar`'s body. 33: wire the forum view at `opens_timeline`; `ChannelEntry::joined` for Join/Leave Thread. 35: fill `invite`, `notification_settings`, `mute_*`, `leave_guild` (MuteChannel/MuteGuild with duration submenus). 36: fill `server_settings`, `create_channel` (category "+" passes the category id), `edit_channel`, `delete_channel`; `sidebar.header` carries the `can_*` flags. To confirm on real traffic: a PATCH with only `channel_overrides.collapsed` merges into existing overrides.
