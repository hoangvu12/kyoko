# 18 — Voice message and audio playback

**What to build:** Play voice messages (`IS_VOICE_MESSAGE` attachments with `waveform` and `duration_secs`) in Discord's pill: play and pause, the waveform from the payload with the played part in `--accent-2`, time, playback speed. Play ordinary audio attachments with the audio player under their file card. Recording is out of scope. This needs audio decoding (Ogg Opus at least) and output: check crates.io, prefer pure Rust, add to `[workspace.dependencies]`, keep it out of the headless crates.

**Blocked by:** 05

**Status:** resolved

**Mockups:** `docs/design/mockups/voice-message.html`

**Spec:** `docs/design/midnight.md`, Composer extras

- [x] A voice message and an audio file play, pause, seek and change speed
- [x] Only one plays at a time; switching channel stops it
- [x] Audio is fetched through the existing media pipeline (no new network path)

## Answer

Merged to main as `3cd21bd..056cc8f` (the first two commits don't build `--locked` alone; the head does). Docs: `docs/media.md` (Audio), `docs/ui.md`, `docs/architecture.md`, `docs/parity.md`, `THIRD_PARTY_NOTICES.md`.

- New crate `crates/audio` (`kyoko-audio`, no kyoko dependencies; no headless crate gained one): Ogg Opus via `ogg` 0.9.2 + `opus-decoder` 0.1.1 (pure Rust, RFC 8251 vectors), Vorbis/MP3/FLAC/WAV/AAC-M4A/Matroska via `symphonia` 0.6.1, output via `cpal` 0.18.2 (uses the pinned `windows` 0.62). Decoding under `catch_unwind` (bad files error, never panic). A `Deck` plays one track at a time, resampled to the device rate, 1.5x/2x keep pitch (WSOLA). Outputs: real device (opened only while playing), a silent clock (mock runs), a manual clock (tests, captures). Clips are decoded whole, capped at 96 MiB decoded (about 17 min mono / 8 min stereo at 48 kHz); longer shows an error.
- Media pipeline: `MediaService::audio` with `Purpose::Audio` (same fetcher, allowlist, 64 MiB cap, dedupe, disk cache); per-request headers, audio gets Chrome's `<audio>` headers (`Fingerprint::audio_headers`), no Authorization, no cookie; 403/404/410 on a signed link refreshes it through `Gallery::refresh`.
- `bridge::AudioPlayer`: play/pause, seek, speed, mute, duration probe for files (fetches up to 16 MiB on first show); starting a track pauses the other (each keeps its place); `left(channel)` and `stop()`. Helpers `waveform` (bad or missing draws flat), `is_voice_message`, `is_audio`, `audio_clock`, `local_audio`.
- Desktop `timeline/voice.rs`: the Midnight pill (play, 40 waveform bars with the played part in `accent_2`, time, 1x/1.5x/2x chip, click the waveform to seek); audio files get a 400-wide card with play, time, seek bar, mute (no volume slider; loading is a dimmed play button). `Listening` on `Pictures`, shared by the main timeline and the thread pane. Channel switch calls `left`; account switch closes the timeline; sign-out calls `stop()` in `release_media`.
- Mockcord: two Opus voice messages and an MP3 fixture, a `post_audio` control, voice attachments on the CDN and `/media` routes, upload descriptors carry `duration_secs`/`waveform`, a `showcase: voice messages` section just before the reactions section.
- Tests (bridge, against mockcord): `a_voice_message_comes_through_the_media_pipeline_and_plays_pauses_seeks_and_speeds_up` (one CDN request with audio headers, replay from cache), `only_one_track_plays_at_a_time_and_each_keeps_its_place`, `leaving_the_channel_stops_playback`, `a_broken_file_shows_an_error_and_a_bad_waveform_draws_flat`. No test plays sound.
- Checks re-run by the orchestrator in lane C: all green. Mockup parity (port 8493): voice-message 13.3% (frozen manual clock, playing pill at 0:04; older showcase messages above); timeline 17.5% (17.6% before, no regression).
- For later tickets: recording needs an input stream (cpal has capture), an Opus encoder (no pure-Rust one today), Ogg muxing (`ogg` writer), a waveform of up to 256 bytes base64, the upload with `duration_secs`/`waveform` and the `IS_VOICE_MESSAGE` flag (mockcord stores both), a `SEND_VOICE_MESSAGES` check and composer UI. Streaming decode would remove the length cap. `Listening::anchor(id)`, `TimelineView::scroll_to_end` and the `voice_shots.rs` click helpers are reusable in captures. New showcase sections go before the reactions section or the reactions captures change.
