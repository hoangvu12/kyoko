# 98 — Join from the sidebar, voice panel, mute and deafen

**What to build:** `Command::JoinVoice` and `LeaveVoice`, plus self mute and deafen. Op 4 (with `preferred_region(s)` and `flags` as decided in 96), waiting for VOICE_STATE_UPDATE and VOICE_SERVER_UPDATE, then 97's connection. Clicking a voice row joins it; clicking the channel you are already in opens the call view placeholder. A voice panel above the user panel; mute and deafen in the user panel become real.

Plan ticket V2 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 97

**Status:** claimed

- [ ] Session test: joining a guild voice channel sends op 4 with that guild and channel id (never a DM), connects to the announced endpoint, and `state.voice()` shows it connected; moving channels reconnects with the new token; Disconnect sends op 4 with `channel_id: null`
- [ ] Mute stops sending and resends op 4 `self_mute`; deafen also stops playback and implies mute; server mute and deafen from VOICE_STATE_UPDATE show
- [ ] A full channel (no events) times out with a toast; the gateway reconnecting re-sends op 4
- [ ] In-process UI test: the row click joins, the panel shows the channel and Disconnect works; captures checked by eye
