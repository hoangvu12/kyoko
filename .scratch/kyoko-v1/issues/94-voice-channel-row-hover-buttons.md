# 94 — Voice channel rows: Discord's hover buttons

**What to build:** From the user after the first real-account run (2026-10-28). Hovering a voice channel row in Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/c_Image_1.png`) shows three buttons at the right: Open Chat (the voice channel's text chat), Invite to Voice (invite dialog for that channel), Edit Channel (gear, only with Manage Channels). Clicking the row itself joins voice (that's the voice tickets from the voice plan; until they land, a row click keeps today's behaviour). Adapt the look to Midnight, don't copy Discord's colours.

**Blocked by:** none

**Status:** resolved

- [x] Voice rows show Open Chat, Invite to Voice and Edit Channel on hover (and while focused), with tooltips; Edit Channel only with Manage Channels; text rows keep their own hover buttons
- [x] Open Chat opens the voice channel's text chat; Invite opens kyoko's invite dialog for that channel (v1 35); Edit Channel opens channel settings (95; until 95 lands, the existing server settings page or nothing, but never a dead button)
- [x] In-process UI test for the buttons; capture checked by eye

## Answer

Merged to main as `b8c301e` (worker `b83e41a1`, lane A; full verify re-run by the orchestrator: all green; kyoko-only captures `channel-sidebar-voice-hover` (new) and `channel-sidebar` checked by eye against c_Image_1.png). Docs: `docs/ui.md` (sidebar), `docs/parity.md`.

- Root cause: voice rows had only 95's gear; after 98 made a row click join voice, the voice channel's text chat was reachable only by joining.
- Voice and stage rows in `channel_sidebar.luau` show three buttons on hover, while the row or a button has keyboard focus, and while selected, each with a tooltip: Open Chat (`chat`, `sidebar.select_channel`, opens the text chat without joining), Invite to Voice (`user_plus`, `sidebar.invite`, 35's dialog; only with the header's `can_invite`), Edit Channel (95's gear, `sidebar.edit_channel`, only when `manageable`). Text rows keep only their gear; the row click still joins.
- Tests in `apps/desktop/src/ui/scripts/voice.rs`: `a_voice_rows_hover_buttons_open_chat_invite_and_edit_without_joining`, `edit_channel_and_invite_need_their_rights_on_a_voice_row` (both fail on the old sidebar).
- Deviations: moving the pointer off a focused row hides the buttons (zgx doesn't say whether focus came from keyboard or click); Invite uses the server-level `can_invite`, like the channel menu.
- For later: ids `channel_chat_<id>`, `channel_invite_<id>`, `channel_gear_<id>`, group `channel_buttons_<id>`. zgx focus and blur bubble, so a row's `on_focus`/`on_blur` covers its buttons; tests focus with `app.ui.input.focus(&app.ui.scene, Some(node))`; tooltips show in `app.texts()` inside `wait_until`. Capture `channel-sidebar-voice-hover` points at (200, 459).
