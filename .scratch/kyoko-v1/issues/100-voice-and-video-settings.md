# 100 — Voice & Video settings

**What to build:** The Settings page ungreyed: devices, volumes, mic test, voice activity or push to talk with a hotkey, input sensitivity (auto or threshold), noise suppression. Settings persist in `store::VoiceSettings`.

Plan ticket V4 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 98

**Status:** resolved

- [x] Session tests: a device change while connected restarts audio without reconnecting; the threshold gates sending; push to talk sends only while held; noise suppression toggles the chain
- [x] Mic test runs without joining; the level meter updates; settings survive a restart
- [x] Captures checked by eye; a page text added to `SETTINGS_PAGES` for the load check

## Answer

Merged to main as `9ca4777` and `be78bfb` (worker `fbc19d34`, lane B; full verify re-run by the orchestrator: green apart from an sccache dropout, the known `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus` flake, and 93's `media_queue::a_burst_of_images_to_one_host_shares_one_http2_connection` failing 1 in 5 (ticket 110); kyoko-only captures `settings-voice`, `settings-keybinds` checked by eye). Docs: `docs/voice.md` ("Settings"), `docs/ui.md`, `docs/desktop.md`, `docs/parity.md`.

- Root cause: `store::VoiceSettings` existed but the session read only the two device ids once at connect; volume, sensitivity, push to talk and noise suppression never reached the connection; no mic test or hotkey; the page was a greyed note.
- Voice crate: a device change reopens only that stream (`audio_devices()`, `audio_restarts()`); `input_level_db()` after volume and denoise; `VoiceStats::frames_denoised`; `MicCheck` in `diagnostics.rs` (oxidcord port: mic through Opus back to output, level every 50 ms, manual mode via `feed_input`/`take_output`). State `MicTest { level_db }`.
- Session: `ApplyVoiceSettings` (reads the store, applies live), `SetPushToTalk(bool)`, `StartMicTest`, `StopMicTest`; a new call gets every setting; in a call the mic test reads the call's own level; join, leave or move ends an outside test; `Session::mic_check()` for tests.
- Bridge setting keys `voice.input_device`, `voice.output_device`, `voice.input_volume`, `voice.output_volume` (0-200), `voice.push_to_talk`, `voice.auto_sensitivity`, `voice.sensitivity_db` (-100..0), `voice.noise_suppression`, `keybinds.push_to_talk`. Desktop `ui/voice_settings.rs` publishes `voice.devices` (listed off the UI thread; "Mock Microphone"/"Mock Speakers" under `--mock`) and `voice.mic_test`; leaving the page stops a test; `integration.rs` registers the push-to-talk hotkey only in Push to Talk mode.
- Luau: `settings/voice.luau` (Voice, Voice Processing; echo cancellation shown as not available), keybind recorder moved to `settings/keybind.luau` (Keybinds lists Push to Talk too).
- Tests: `crates/session/tests/session/voice_settings.rs` (device change without reconnect, threshold gating, push to talk held and across restart, noise suppression toggles the chain, mic test without joining, mic test in a call), `ui::scripts::voice_settings::the_voice_page_tests_the_mic_and_writes_every_voice_setting`.
- Deviations: noise suppression defaults off; automatic sensitivity is a fixed -50 dB (Discord's adaptive gate not ported); the meter moves only during a mic test; push to talk registers a Windows hotkey, so while bound the key combination doesn't reach other apps (Discord uses a keyboard hook); no camera rows (104); no noise suppression button in the voice panel (no icon).
- For later: 104 adds camera rows under a new heading in `settings/voice.luau` (`VoiceSettings.camera_device`, `video_preset` exist); a panel noise suppression button needs only an icon and `settings.set { key = "voice.noise_suppression" }`.
