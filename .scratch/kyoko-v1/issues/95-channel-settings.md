# 95 — Channel settings (Edit Channel)

**What to build:** From the user after the first real-account run (2026-10-28). "Edit Channel" in kyoko's channel menu is a placeholder (`later("edit_channel", ...)` in `apps/desktop/ui/shell/channel_sidebar.luau`). Discord's channel settings (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/c_Image_2.png`, a voice channel): a full-window page with a left nav (channel name and type; Overview, Permissions, Invites, Integrations; Delete Channel) and Esc to close. Overview for a voice channel: Channel Name, Slowmode, Content Visibility (Default, Spoiler, Age-Restricted), Bitrate (8–96 kbps, more with boosts), Video Quality (Auto, 720p), User Limit, Region Override; text channels: name, topic, slowmode, age-restricted, announcement, hide after inactivity. Midnight look, same page pattern as kyoko's server settings (v1 36, Midnight 23).

**Blocked by:** none

**Status:** resolved

- [x] Channel settings page for text and voice channels, opened from Edit Channel (menu and the voice row's gear), gated by Manage Channels; fields as listed, saved with `PATCH /channels/{id}` (only changed fields, with `X-Audit-Log-Reason` where kyoko already sends one), unsaved-changes bar as in server settings
- [x] Permissions (role/member overwrites list and editor, reusing server settings' role permission editor where possible), Invites (list, revoke), Delete Channel through kyoko's confirmation; Integrations listed read-only or left out (say which)
- [x] Session tests for the PATCH body and permission gating; captures checked by eye

## Answer

Merged to main as `cf4e106` (worker `3d094e4f`, lane A; two commits squashed by the orchestrator so each builds; checks re-run: all green; rebased onto 96, rebuilt; kyoko-only captures `channel-settings-overview`, `-voice`, `-permissions`, `-invites` and `channel-sidebar` checked by eye). Docs: `docs/ui.md`, `docs/rest.md`, `docs/parity.md`.

- Root cause: Edit Channel was the `later(...)` stub; the moderation layer's small dialog only knew name, topic, slowmode, age-restricted and category; Manage Channels was checked at server level only; no voice fields, channel invites or regions.
- Opened from Edit Channel and a hover gear on channel rows (voice rows too), both shown only with Manage Channels on that channel after overwrites (sidebar rows carry `manageable`); the session checks the same before any request. Server settings' frame (Midnight 23), search off, left nav: name and type, Overview, Permissions, Invites, Delete Channel.
- Overview: name, slowmode; text: topic, Age-Restricted, Announcement (only with `NEWS`), Hide After Inactivity; voice: Content Visibility (Default / Spoiler `1 << 21` / Age-Restricted), bitrate 8 kbps to the boost-tier limit (96/128/256/384, stage 64), Video Quality, user limit (∞ to 99), Region Override from `GET /guilds/{id}/regions`; Category select. Save sends `PATCH /channels/{id}` with only changed fields; no `X-Audit-Log-Reason` (kyoko sends one only for kick, ban, timeout).
- Permissions: overwrite list (@everyone first), "+" with search, tri-state editor reusing the role editor's list trimmed to channel rights; roles at or above your highest are read-only. Invites: `GET /channels/{id}/invites`, copy, revoke with confirmation. Delete through the existing confirmation, then `bridge::replacement_channel`.
- Not done: Integrations (no webhooks or followed channels in kyoko); Discord's Private Channel basic view; moving a channel out of a category; bitrate marks only at the ends.
- For later: `Command::EditChannel { channel_id, changes: ChannelChanges }`, `bridge::ChannelForm::changes`, `state::bitrate_limit` / `user_limit_range`; new `LoadChannelInvites`, `RevokeChannelInvite`, `LoadVoiceRegions`, state `channel_invites`, `voice_regions`; Luau `kit.range`, `kit.radios` notes, `dialog.frame { searchable = false }`. Content below the fold isn't clickable in Luau tests (use the 1400 px window).
