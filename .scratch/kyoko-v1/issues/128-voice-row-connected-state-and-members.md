# 128 — Voice channel row: connected state, mute/deafen rules, members

**What to build:** From the user's real run 4 (2026-10-30), screenshots in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy).

- image-12 (Discord): while you are connected, the voice channel row shows it: green speaker icon, the elapsed time, a "Set a channel status" line and an "Invite to Voice" row. image-13 (kyoko) shows none of this.
- Mute and deafen follow Discord's state machine, in the user panel, the voice panel and the member's icons under the channel: mute shows muted; deafen shows deafened and muted; deafen while muted, then undeafen, stays muted; deafen when not muted, then undeafen, is unmuted. Kyoko shows only deafen today (image-13), not mute. The voice state sent to Discord (op 4 `self_mute`/`self_deaf`) follows the same rules.
- image-02: clicking a user under a voice channel in the sidebar opens their profile popout (Discord does; kyoko does nothing).
- image-16: voice channel members show the server-profile name but not the server-profile avatar; use the server avatar when the member has one (81 has the member data).

Builds on 98 (voice panel, mute/deafen), 94 (voice row hover buttons), 101 (call view), 81/92 (server profiles, popout).

**Blocked by:** none

**Status:** claimed (lane C)

- [ ] Connected voice row shows the green icon, elapsed time, "Set a channel status" (setting it sends the web client's request, or opens nothing if not built yet: say which) and "Invite to Voice"
- [ ] Mute/deafen state machine as above; session tests prove the op 4 sequence for each of the four cases
- [ ] Member icons under the channel show mute and deafen separately, for you and for others
- [ ] Clicking a voice member opens their profile popout
- [ ] Voice members use their server-profile avatar when they have one
- [ ] In-process UI tests; kyoko-only captures checked by eye against image-12, -13, -16
