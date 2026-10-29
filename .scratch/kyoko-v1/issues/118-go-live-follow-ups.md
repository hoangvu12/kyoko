# 118 — Go Live follow-ups: stream audio, pause, own preview, source swap

**What to build:** Follow-up from 107. Going live works (op 18, screen layer, DAVE, stop), but it is short of Discord in four ways:

1. **Stream audio send.** Discord shares the app's or screen's sound with the stream. The stream connection is `AudioIo::Manual` today. Capture the shared window's process audio (WASAPI process loopback, Windows 10 2004+) or the whole system output when a screen is shared (excluding kyoko's own output), Opus-encode it and send it on the stream connection. 113's `AudioIo::Playback` covers the viewer side. Picker gets Discord's "Sound" toggle for the source.
2. **Pause trigger.** `Command::SetStreamPaused` exists; Discord pauses when the shared window is minimised and shows the paused state to viewers. Wire it from the capture (window minimised / restored).
3. **Own stream preview.** Discord shows your own stream as a tile in the call view (and lets you watch it full-window). Show it from the local frames, no network.
4. **Source swap.** Discord's "Change Windows" switches source without ending the stream. Swap the capture behind the same stream connection instead of op 19 + op 18.
5. **Watched stream audio in the echo reference.** From 116: stream audio you watch plays from the stream connection's mixer, outside the call's AEC render reference, so on speakers it echoes into the call. Mix it into the call connection's render reference.

**Blocked by:** 107

**Status:** claimed

- [ ] Session tests (fake capture + fake loopback audio): stream audio reaches a scripted viewer DAVE-encrypted; minimise pauses (op 22) and restore resumes on a keyframe; swapping source keeps the same stream connection and key
- [ ] Tests and captures never capture the real screen, real system audio or other people's windows
- [ ] Captures of the call view with your own stream tile and of the picker with the Sound toggle, checked by eye
- [ ] `docs/voice.md` updated

## Decision (user, 2026-10-29)

"Do like Discord": sharing a **screen** shares the whole computer's sound **minus kyoko's own** (so people in the call don't hear their own voices echoed back); sharing an **app** shares only that app's sound (its process tree). On Windows this is WASAPI process loopback (`ActivateAudioInterfaceAsync` with `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`): `PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE` on kyoko's own PID for a screen, `INCLUDE_TARGET_PROCESS_TREE` on the window's PID for an app. Sound on by default, with Discord's toggle in the picker. Stream audio bypasses mic noise suppression, echo cancellation and gain control.
