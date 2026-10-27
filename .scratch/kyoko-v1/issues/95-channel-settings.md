# 95 — Channel settings (Edit Channel)

**What to build:** From the user after the first real-account run (2026-10-28). "Edit Channel" in kyoko's channel menu is a placeholder (`later("edit_channel", ...)` in `apps/desktop/ui/shell/channel_sidebar.luau`). Discord's channel settings (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/c_Image_2.png`, a voice channel): a full-window page with a left nav (channel name and type; Overview, Permissions, Invites, Integrations; Delete Channel) and Esc to close. Overview for a voice channel: Channel Name, Slowmode, Content Visibility (Default, Spoiler, Age-Restricted), Bitrate (8–96 kbps, more with boosts), Video Quality (Auto, 720p), User Limit, Region Override; text channels: name, topic, slowmode, age-restricted, announcement, hide after inactivity. Midnight look, same page pattern as kyoko's server settings (v1 36, Midnight 23).

**Blocked by:** none

**Status:** claimed

- [ ] Channel settings page for text and voice channels, opened from Edit Channel (menu and the voice row's gear), gated by Manage Channels; fields as listed, saved with `PATCH /channels/{id}` (only changed fields, with `X-Audit-Log-Reason` where kyoko already sends one), unsaved-changes bar as in server settings
- [ ] Permissions (role/member overwrites list and editor, reusing server settings' role permission editor where possible), Invites (list, revoke), Delete Channel through kyoko's confirmation; Integrations listed read-only or left out (say which)
- [ ] Session tests for the PATCH body and permission gating; captures checked by eye
