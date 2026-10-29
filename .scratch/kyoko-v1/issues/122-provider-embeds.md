# 122 — Provider embeds and players (from oxidcord fc1dc43)

**What to build:** Discord's playable link embeds: YouTube, Vimeo, Spotify and SoundCloud links show their preview card and, on play, the provider's real player inside the message. Port from oxidcord `fc1dc43` ("Add chat media player and provider embeds": `crates/ui/src/webframe.rs` hosts a sandboxed frame through `wry`, i.e. WebView2; `crates/media/src/providers.rs`), adapted to kyoko (zgx window, Luau UI, Rust timeline above the Luau tree). Only the provider-embed part: kyoko already plays video/audio attachments (91), so skip upstream's player crate, FFmpeg codec pack and symphonia, and the reply-mention toggle / inbox / help buttons unless kyoko lacks them (report).

**Status:** claimed (lane C)

## Decision (user, 2026-10-29)

Use WebView (WebView2 via `wry`, already on Windows 11; nothing bundled). Exe grows ~1-2 MB; memory only while a player is open. The WebView is created only when the user presses play, and torn down when the player closes or scrolls far off screen.

- [ ] Play on a YouTube/Vimeo/Spotify/SoundCloud embed opens the provider's embed player in place (Discord's sizes and controls); closing or scrolling away destroys it; one live player at a time unless Discord allows more
- [ ] Isolation: a separate WebView2 user-data folder; no Discord cookies, token or headers ever reach the frame; navigation limited to the provider's embed origins (others open in the system browser); no kyoko IPC from page script beyond what's needed to size/close; no autoplay before the click
- [ ] Missing WebView2 runtime: the card falls back to opening the link in the browser, with a note
- [ ] The frame sits correctly with the overlay rule (Rust timeline above Luau; dialogs and popouts over the timeline hide or clip it)
- [ ] Tests: provider detection and embed URLs at the session/bridge seam against mockcord fixtures; the frame host behind a seam (no real WebView or network in tests); a hand run may load one public YouTube embed on `--mock` only, stopped by PID
- [ ] Exe size and idle/playing memory reported; kyoko-only captures of the card and the open player checked by eye
- [ ] Commit bodies end `Ported from oxidcord fc1dc43...` (full sha)