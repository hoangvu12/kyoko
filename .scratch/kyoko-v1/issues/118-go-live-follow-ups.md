# 118 — Go Live follow-ups: stream audio, pause, own preview, source swap

**What to build:** Follow-up from 107. Going live works (op 18, screen layer, DAVE, stop), but it is short of Discord in four ways:

1. **Stream audio send.** Discord shares the app's or screen's sound with the stream. The stream connection is `AudioIo::Manual` today. Capture the shared window's process audio (WASAPI process loopback, Windows 10 2004+) or the whole system output when a screen is shared (excluding kyoko's own output), Opus-encode it and send it on the stream connection. 113's `AudioIo::Playback` covers the viewer side. Picker gets Discord's "Sound" toggle for the source.
2. **Pause trigger.** `Command::SetStreamPaused` exists; Discord pauses when the shared window is minimised and shows the paused state to viewers. Wire it from the capture (window minimised / restored).
3. **Own stream preview.** Discord shows your own stream as a tile in the call view (and lets you watch it full-window). Show it from the local frames, no network.
4. **Source swap.** Discord's "Change Windows" switches source without ending the stream. Swap the capture behind the same stream connection instead of op 19 + op 18.
5. **Watched stream audio in the echo reference.** From 116: stream audio you watch plays from the stream connection's mixer, outside the call's AEC render reference, so on speakers it echoes into the call. Mix it into the call connection's render reference.

**Blocked by:** 107

**Status:** resolved

- [x] Session tests (fake capture + fake loopback audio): stream audio reaches a scripted viewer DAVE-encrypted; minimise pauses (op 22) and restore resumes on a keyframe; swapping source keeps the same stream connection and key
- [x] Tests and captures never capture the real screen, real system audio or other people's windows
- [x] Captures of the call view with your own stream tile and of the picker with the Sound toggle, checked by eye
- [x] `docs/voice.md` updated

## Decision (user, 2026-10-29)

"Do like Discord": sharing a **screen** shares the whole computer's sound **minus kyoko's own** (so people in the call don't hear their own voices echoed back); sharing an **app** shares only that app's sound (its process tree). On Windows this is WASAPI process loopback (`ActivateAudioInterfaceAsync` with `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`): `PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE` on kyoko's own PID for a screen, `INCLUDE_TARGET_PROCESS_TREE` on the window's PID for an app. Sound on by default, with Discord's toggle in the picker. Stream audio bypasses mic noise suppression, echo cancellation and gain control.

## Answer

Merged `5e4eead` (lane B, worker `aadc5298`), verified by the orchestrator: all done-means steps green; tls-capture chrome-156 0/0; kyoko-only captures `voice-going-live` (own stream tile, LIVE), `voice-go-live-preview` (Sound switch), `voice-call` checked by eye.

- Stream sound: `crates/voice/src/sound.rs` + `sound/loopback.rs`, WASAPI process loopback (`VAD\Process_Loopback`, 48 kHz 16-bit stereo, event driven). Screen = `SoundTarget::Everything { except: own pid }` (EXCLUDE tree), app = `SoundTarget::App { process }` (INCLUDE tree, pid from `GetWindowThreadProcessId`, `ScreenTarget.process`); kyoko's own window or an unknown pid shares no sound. Own path in media.rs: stereo Opus (Audio, 128 kbps, FEC), DAVE, stream connection's audio SSRC; skips AEC/NS/AGC/volume/gate; op 5 speaking 2. Picker Sound switch (`go_live_sound`, default on), `Command::GoLive { sound }`. Loopback failure: stream goes on silent (log warning). Seam `session::FakeCapture` in `VoiceSetup.capture` (tests and `--mock`, 440 Hz tone). `PROPVARIANT`'s Drop in the windows crate frees VT_BLOB params: keep stack-owned ones in `ManuallyDrop`.
- Pause: `VideoSource::hidden()` (IsIconic) -> `VoiceEvent::VideoSourceHidden` -> `SetStreamPaused` (op 22); pauses video and sound (5 silence frames, op 5 0); resume on a keyframe. "Your stream is paused" / "Stream paused" (`CallStream.paused`).
- Own stream tile from local frames (`stream_feed`), hover Full Window / Change Windows / Stop Streaming, not auto-focused.
- Source swap: `VoiceConnection::replace_screen` on the same connection (no op 19, no second op 18, op 12 with the new max_resolution, keyframe, sound retargeted).
- Watched stream audio: the watched stream connection is always `AudioIo::Manual` and hands mixed frames to the call (`PlaybackFeed`, `VoiceConnection::play_through`, max 5 queued), mixed after master volume, so it plays through the call's device and is in the AEC reference (~71 dB ERLE). `AudioIo::Playback` / `AudioEngine::playback` from 113 removed.
- Also fixed: a late frame from the detached video-send thread could bring back a ghost own preview after `stop_camera`.
- Tests: `go_live::going_live_shares_the_windows_sound_dave_encrypted_and_minimising_pauses_it`, `go_live::changing_windows_swaps_the_capture_on_the_same_stream_connection_and_key`, `voice_processing::a_watched_streams_sound_plays_through_the_call_and_is_cancelled_from_your_microphone`, plus UI script tests. Hand run: `cargo run -p kyoko-voice --example own_sound` (INCLUDE on its own pid, own tone). EXCLUDE mode and real minimise detection were never run for real: check on 102.
- Not done: Nitro gating, a separate stream-sound mute; Discord's real stream-audio bitrate unknown.
