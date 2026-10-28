# 113 — Go Live: stream audio and a full viewer

**What to build:** Follow-up from 106. Watching a Go Live stream shows the video in the focused call tile, but the stream's audio isn't played (the stream connection runs muted and deafened so it never sends media) and there's no full-window viewer. Discord plays the streamer's shared audio with its own volume control and lets you pop the stream out to fill the window or go fullscreen. Play the stream connection's received Opus through the mixer with a per-stream volume (persisted like 101's user volume, separate from the person's voice volume), keep the connection sending nothing, and add a full-window stream view (reuse 89's viewer overlay pattern) with Stop Watching, volume and exit.

**Blocked by:** none

**Status:** resolved

- [x] Session test: a scripted streamer's Opus on the stream connection reaches the mixed output at the stream volume; the stream connection still sends no media; deafen silences it; stopping ends it
- [x] Stream volume persists per streamer, separate from their voice volume
- [x] Full-window stream view opens from the focused stream tile, shows the stream, volume and Stop Watching, and Escape or the exit button returns to the call view; in-process UI test; captures checked by eye

## Answer

Merged to main as `d5dec3c` and `3d814cb` (worker `d9d6b49d`, lane A; full verify re-run by the orchestrator: all green, tls-capture 0/0 against chrome-156, kyoko-only captures `voice-stream-viewer`, `voice-stream-watching`, `voice-stream` checked by eye). Docs: `docs/voice.md` (Go Live, tests), `docs/ui.md` (call view), `docs/parity.md`.

- Root cause: the stream connection was hard-wired to `AudioIo::Manual` and deafened, so its Opus was decoded and zeroed; no volume of its own; the only viewer was the focused tile.
- Voice: `AudioIo::Playback { output }` and `AudioEngine::playback`, an output-only engine that never opens a mic; the stream connection uses it (`Manual` under tests and `--mock`) and stays muted, so it sends no media.
- State: `UserAudio.stream_volume` (default 100, 0-200) and `stream_gain()`, saved in the same `voice.user_audio` kv list, separate from the voice volume.
- Session: `Command::SetStreamVolume { user_id, volume }`; the stream connection gets the owner's stream gain, the output volume and `set_deaf` (self and server deafen, output device and volume changes follow). Op 12 on the stream connection already maps `audio_ssrc` to the owner. Mockcord `MockStream::audio_ssrc()`.
- UI: bridge `CallStream.volume`, action `voice.stream_volume`. `call_view.luau`: Full Window button (`call_stream_full_<id>`) beside Stop Watching on a watched tile; a full-window `ui.modal` (`stream_viewer`, sized from the settings dialog's window ref as `dialog.popup` does) with the stream `fit=contain`, a name pill with LIVE, and a bar with Stream Volume (`stream_viewer_volume`), Stop Watching and Exit Full Window; Escape or exit returns, and it closes by itself when watching stops. The volume slider is shared between User Volume and Stream Volume (prop `slider` for the inner id).
- Tests: `streams::a_watched_streams_audio_plays_at_its_own_volume_sends_nothing_and_follows_deafen` (level halves at 50, deafen silences, 0 packets from the stream SSRC, nothing after stop, both volumes persist across restart), `ui::scripts::voice::the_full_window_stream_view_shows_the_stream_sets_its_volume_and_closes`. Capture screen `voice-stream-viewer` (`--streams 1`).
- Deviations: stream audio plays through the stream connection's own output stream (WASAPI mixes it with the call's), not the call mixer; no separate stream mute (Discord has one); "full window", not OS fullscreen; the modal covers the window controls while open, as the lightbox does.
- For later: 107 keeps `AudioIo::Playback` for viewers and needs a capture mode when sending. zgx: a `ui.component` with an `id` prop gives the component node that id too, so name inner-id props differently; an effect syncing a modal's `open` must read it with `:peek()` or Escape is undone. Captures can't look up Luau ids from Rust; `voice-stream-viewer` clicks 43 px left of the "Stop Watching" text (`FULL_WINDOW_FROM_STOP`).
- Process note: the worker's squash dropped tickets 115 and 116 that landed on main meanwhile; restored in `9fc213c`. Workers must not squash across origin/main.
