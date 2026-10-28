# 101 — Call view, speaking rings and per-user controls

**What to build:** `bridge::voice` participants and speaking; the call view's audio tiles; speaking rings in the sidebar rows; the participant menu with user volume and local mute; the Opus bitrate follows the channel's `bitrate`.

Plan ticket V5 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 98

**Status:** resolved

- [x] Session test: op 5 from a scripted member shows that user speaking and stops after silence; op 11 and 13 add and remove tiles
- [x] User volume scales that user's mixed output (manual sink); local mute silences it
- [x] Opening the voice channel you are in shows the call view; Open Chat shows its text chat

## Answer

Merged to main as `ba05e62` and `d681844` (worker `cf01c3ab`, lane B; full verify re-run by the orchestrator: all green; `tls-capture --compare chrome-156` 0 and 0; kyoko-only captures `voice-call`, `voice-panel`, `channel-sidebar` checked by eye, no mockup exists). Docs: `docs/voice.md`, `docs/ui.md`, `docs/parity.md` (`voice-call`).

- Root cause: nothing tracked who was in the call or speaking beyond op 5 (sent only when speech starts, so a ring would never end); op 11 was dropped; the mixer's per-user volume had no setter or storage; the encoder was fixed at 64 kbps; the voice row click opened the text chat as a placeholder.
- Voice core `talk.rs`: op 5 with a non-zero flag starts speaking at once; op 5 with 0 or op 13 ends it; mixed audio above -50 dBFS keeps it on; 400 ms without (silence, DTX, no packets) ends it. Your own speech marks you speaking too. Opus bitrate follows the channel's `bitrate` at connect and on CHANNEL_UPDATE, clamped 6-510 kbps (a fresh-reconnect bug that kept 64 kbps was fixed).
- State/session: `VoiceSession::peers` (op 11/13), `UserAudio { volume, muted }` via `State::user_audio`; `Command::SetUserVolume` (0-200%) and `SetUserMuted`, saved per account in kv `voice.user_audio` (bad entries skipped one by one; never for yourself), applied when a connection comes up.
- Bridge `voice_call` (tiles from guild voice states, op 11 peers and you, by name; works without a guild for 108), `voice_speaking` with `SpeakingPace` (on change, at most every 100 ms). Desktop `src/ui/voice_call.rs` publishes `voice.call`, `voice.speaking`; actions `voice.open_chat`, `voice.user_volume`, `voice.local_mute`; `voice.open_call` and a click on your own voice row open the call view; the Rust timeline slot is hidden while it's open (third cover beside forum and hidden-channel).
- Luau `shell/call_view.luau`: header with name and Open Chat; 16:9 tiles with avatar, name pill with mute/deafen/local-mute icons, green speaking ring; click focuses; right-click on someone else opens User Volume (applies on release) and Mute; bottom bar camera and screen (disabled until 104/105), mute, deafen, Disconnect. Sidebar voice rows get only a speaking ring overlay. New size tokens `call_tile_avatar`, `call_tile_min_width`, `call_control_size`, `call_ring_width`, `call_menu_width`.
- Tests: session `voice::a_scripted_member_speaks_until_silence_and_op_11_and_13_add_and_remove_peers`, `voice::user_volume_scales_a_member_local_mute_silences_them_and_both_persist` (also bitrate 96 -> 128 kbps); bridge `voice::peers_from_op_11_and_13_come_and_go_as_tiles_with_their_volume_and_speaking`, `voice::speaking_is_published_on_change_at_most_every_100_ms`; Luau `ui::scripts::voice::the_call_view_shows_the_tiles_and_its_controls_and_menu_reach_the_host`, `a_speaking_member_gets_a_ring_in_the_sidebar`.
- Deviations: avatar tiles only; the participant menu is on call tiles only (no sidebar row menu); joining doesn't open the call view by itself.
- For later: 103/104 add video to `bridge::CallTile.video` (tile ids `call_tile_<user>`, size from `size()` in `call_view.luau`); 106 stream tiles go in the same list; 100 can read `voice.call` and `voice.speaking`. The `voice-call` capture feeds a tone into the manual input; mockcord members need `set_dave_version(1)` before `send_as` reaches kyoko.
