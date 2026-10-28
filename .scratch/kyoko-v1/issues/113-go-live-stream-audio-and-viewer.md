# 113 — Go Live: stream audio and a full viewer

**What to build:** Follow-up from 106. Watching a Go Live stream shows the video in the focused call tile, but the stream's audio isn't played (the stream connection runs muted and deafened so it never sends media) and there's no full-window viewer. Discord plays the streamer's shared audio with its own volume control and lets you pop the stream out to fill the window or go fullscreen. Play the stream connection's received Opus through the mixer with a per-stream volume (persisted like 101's user volume, separate from the person's voice volume), keep the connection sending nothing, and add a full-window stream view (reuse 89's viewer overlay pattern) with Stop Watching, volume and exit.

**Blocked by:** none

**Status:** claimed

- [ ] Session test: a scripted streamer's Opus on the stream connection reaches the mixed output at the stream volume; the stream connection still sends no media; deafen silences it; stopping ends it
- [ ] Stream volume persists per streamer, separate from their voice volume
- [ ] Full-window stream view opens from the focused stream tile, shows the stream, volume and Stop Watching, and Escape or the exit button returns to the call view; in-process UI test; captures checked by eye
