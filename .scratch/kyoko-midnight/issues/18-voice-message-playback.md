# 18 — Voice message and audio playback

**What to build:** Play voice messages (`IS_VOICE_MESSAGE` attachments with `waveform` and `duration_secs`) in Discord's pill: play and pause, the waveform from the payload with the played part in `--accent-2`, time, playback speed. Play ordinary audio attachments with the audio player under their file card. Recording is out of scope. This needs audio decoding (Ogg Opus at least) and output: check crates.io, prefer pure Rust, add to `[workspace.dependencies]`, keep it out of the headless crates.

**Blocked by:** 05

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/voice-message.html`

**Spec:** `docs/design/midnight.md`, Composer extras

- [ ] A voice message and an audio file play, pause, seek and change speed
- [ ] Only one plays at a time; switching channel stops it
- [ ] Audio is fetched through the existing media pipeline (no new network path)
