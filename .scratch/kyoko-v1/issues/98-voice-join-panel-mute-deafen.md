# 98 — Join from the sidebar, voice panel, mute and deafen

**What to build:** `Command::JoinVoice` and `LeaveVoice`, plus self mute and deafen. Op 4 (with `preferred_region(s)` and `flags` as decided in 96), waiting for VOICE_STATE_UPDATE and VOICE_SERVER_UPDATE, then 97's connection. Clicking a voice row joins it; clicking the channel you are already in opens the call view placeholder. A voice panel above the user panel; mute and deafen in the user panel become real.

Plan ticket V2 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 97

**Status:** resolved

- [x] Session test: joining a guild voice channel sends op 4 with that guild and channel id (never a DM), connects to the announced endpoint, and `state.voice()` shows it connected; moving channels reconnects with the new token; Disconnect sends op 4 with `channel_id: null`
- [x] Mute stops sending and resends op 4 `self_mute`; deafen also stops playback and implies mute; server mute and deafen from VOICE_STATE_UPDATE show
- [x] A full channel (no events) times out with a toast; the gateway reconnecting re-sends op 4
- [x] In-process UI test: the row click joins, the panel shows the channel and Disconnect works; captures checked by eye

## Answer

Merged to main as `8dc8267` and `c2962f3` (worker `584d901b`, lane B; full verify re-run by the orchestrator: all green; `tls-capture --compare chrome-156` 0 and 0 differences on https and wss; kyoko-only captures `voice-panel`, `user-panel`, `channel-sidebar` checked by eye). Docs: `docs/voice.md` ("Joining"), `docs/ui.md`, `docs/fingerprint.md`, `docs/architecture.md`, `docs/parity.md`.

- Root cause: nothing connected the session to `crates/voice`: no join or leave command, op 4 without regions or flags, a voice row only opened its text chat, and mute and deafen only flipped Luau-local icons.
- Session: `task/voice_link.rs` (port of oxidcord's voice_link), `Command::JoinVoice`/`LeaveVoice` plus mute and deafen toggles, `session::VoiceSetup` (`Session::set_voice_setup`, `Session::voice_connection`; audio, trust, join timeout 15 s). Only voice or stage channels in that guild; op 4 never goes out without a guild. Leave sends op 4 with the guild id and `channel_id: null`. A fresh gateway session resends op 4 and reconnects voice.
- Op 4: gateway `VoiceStateRequest` encodes in the docs' key order; `preferred_region`/`preferred_regions` from READY's `geo_ordered_rtc_regions` (protocol `Ready.geo_ordered_rtc_regions`); `flags: 2` (ALLOW_VOICE_RECORDING without CLIPS, unconfirmed until a web capture). No latency pings to discord.media.
- State: `State::voice()` holds status, channel and self flags (kept across joins, `voice_self`); `State::own_voice_state()` gives your server mute and deaf; sidebar members still come from guild voice states. Deafen implies mute; unmuting while deafened undeafens; server mute or deaf also stops sending.
- UI: `bridge::voice_panel`/`voice_controls`, `apps/desktop/src/ui/voice_panel.rs` (actions `voice.join`, `voice.leave`, `voice.toggle_mute`, `voice.toggle_deafen`, `voice.open_call`), `shell/voice_panel.luau` as its own panel between the sidebar and the user panel; the user panel's buttons are real and red when the server holds them; a voice row click is `voice.join` in `channel_sidebar.luau` (`audio` branch).
- Mockcord: VOICE_SERVER_UPDATE only on a channel change (as Discord), tokens `mock-voice-token-<channel>`, `fill_voice_channel`, `VoiceControl::hold_echo`/`release_echo`, READY regions mock-east/central/west. `--mock` forces `AudioIo::Manual`, so nothing plays through the shared desktop.
- Tests: `crates/session/tests/session/voice.rs` (join, move, leave; mute and deafen; server mute and deaf; full channel times out with a toast; re-identify resends op 4), `apps/desktop/src/ui/scripts/voice.rs`.
- Not yet: clicking the channel you're in (or the panel's channel line) opens its text chat as the call-view placeholder (101); no Camera, Go Live or noise suppression row (100, 104, 105); guild channels only (108).
- For later: 101 replaces the placeholder in `Link::join_voice`/`voice.open_call`; `VoiceEvent::Speaking` already feeds `LocalEvent::Speaking`. 100: device settings are read from store at connect time, not applied live. 99 plugs in through `VoiceOptions.transform` in `voice_link::voice_options`. 94: the row click lives in the `audio` branch of `channel_sidebar.luau`.
