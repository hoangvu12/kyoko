# 81 — Server profiles, animated avatars and banners

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Messages and profiles show the global profile: the user's server avatar, nickname and server banner aren't used (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_1.png` kyoko vs `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_2.png` Discord, same message). Animated avatars and banners don't animate (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_3.png` kyoko vs `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_4.png` Discord).

**Blocked by:** none

**Status:** resolved

- [x] Message rows, member list, popouts and full profiles use the member's server avatar, nick and server banner/bio where set (member `avatar`, `banner`; `guild_member_profile` in the profile response), falling back to global
- [x] Animated avatars (`a_` hashes) animate where Discord animates them (on hover in message rows, always in popouts and the user panel, per the media playback setting); animated banners in profiles
- [x] Tests at the bridge seam for the URL choice (server vs global, animated vs static); a kyoko capture checked by eye

## Answer

Merged to main as `69c33d3` (worker `bdca7cbb`, lane A; checks re-run by the orchestrator: all green; parity timeline 14.2%, member-list 14.2%, profile-popout 20.7%, kyoko-only `server-profile` checked by eye; full-profile 59.2% per the worker, from Midnight 20's larger layout). Docs: `docs/media.md`, `docs/rest.md`, `docs/parity.md`.

- Root cause: rows built avatar URLs from the global avatar although member avatars were stored; the profile's `guild_member_profile` was dropped; every request asked for a still image and banner URLs were hard-coded to Discord's CDN.
- `bridge::Avatar` picks server avatar (member list, then profile `guild_member`, then the message's `member`), then global, then default. Used by message rows, reply previews, member list, search, thread starters, the delete dialog and `ProfileCard`.
- Server part of a profile kept per user and server: `State::guild_profile(user_id, guild_id)`. Member banners use `guilds/{g}/users/{u}/banners/{hash}`; facts from docs.discord.food.
- Hover: message and member rows show the still avatar and lay the animated one on top while hovered (Luau avatar component props `moving`/`playing`, rows report `members.avatar_hover`); the media playback setting and reduced motion still apply. Profiles, the status menu and the DM profile column always animate.
- Mockcord serves member avatars, member banners and user banners; some mock users have animated avatars and banners. Test `crates/bridge/tests/bridge/server_profiles.rs`.
- Not done: server avatars in the sidebar voice list, friends and composer suggestions; `theme_colors` and `profile_effect` not read.
- For 92: `ProfileCard.avatar` (`Avatar`) and `.banner` (`ProfileBanner`) give the URLs; `banner_request(cdn)` takes the CDN base. Add `theme_colors`/`profile_effect` to `RawMeta` in `crates/state/src/loaded.rs`, on both the global profile and `GuildProfile`. `bridge::dm_panel` reads `card.loaded` and has no loading state.
