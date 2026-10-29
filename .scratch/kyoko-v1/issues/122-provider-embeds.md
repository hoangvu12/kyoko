# 122 — Provider embeds and players (from oxidcord fc1dc43)

**What to build:** Discord's playable link embeds: YouTube, Vimeo, Spotify and SoundCloud links show their preview card and, on play, the provider's real player inside the message. Port from oxidcord `fc1dc43` ("Add chat media player and provider embeds": `crates/ui/src/webframe.rs` hosts a sandboxed frame through `wry`, i.e. WebView2; `crates/media/src/providers.rs`), adapted to kyoko (zgx window, Luau UI, Rust timeline above the Luau tree). Only the provider-embed part: kyoko already plays video/audio attachments (91), so skip upstream's player crate, FFmpeg codec pack and symphonia, and the reply-mention toggle / inbox / help buttons unless kyoko lacks them (report).

**Status:** resolved

## Decision (user, 2026-10-29)

Use WebView (WebView2 via `wry`, already on Windows 11; nothing bundled). Exe grows ~1-2 MB; memory only while a player is open. The WebView is created only when the user presses play, and torn down when the player closes or scrolls far off screen.

- [x] Play on a YouTube/Vimeo/Spotify/SoundCloud embed opens the provider's embed player in place (Discord's sizes and controls); closing or scrolling away destroys it; one live player at a time unless Discord allows more
- [x] Isolation: a separate WebView2 user-data folder; no Discord cookies, token or headers ever reach the frame; navigation limited to the provider's embed origins (others open in the system browser); no kyoko IPC from page script beyond what's needed to size/close; no autoplay before the click
- [x] Missing WebView2 runtime: the card falls back to opening the link in the browser, with a note
- [x] The frame sits correctly with the overlay rule (Rust timeline above Luau; dialogs and popouts over the timeline hide or clip it)
- [x] Tests: provider detection and embed URLs at the session/bridge seam against mockcord fixtures; the frame host behind a seam (no real WebView or network in tests); a hand run may load one public YouTube embed on `--mock` only, stopped by PID
- [x] Exe size and idle/playing memory reported; kyoko-only captures of the card and the open player checked by eye
- [x] Commit bodies end `Ported from oxidcord fc1dc43...` (full sha)

## Answer

Merged `1ea282b` Play provider embeds in their own player (lane C, worker `233cbcbd`), verified by the orchestrator: all done-means steps green on the first run; tls-capture chrome-156 0/0; kyoko-only captures `timeline-providers` and `provider-player` checked by eye (card with play/open pill; 400x225 player box with a close button; captures use a stand-in host). Isolation code read by the orchestrator.

- `bridge::providers` (headless): `ProviderPlayer::of(embed)` takes the page URL then `video.url`; https only, no userinfo, validated ids only (YouTube 11 chars incl. youtu.be/shorts/live/embed and t/start; Vimeo digits; Spotify 22 alnum incl. intl-xx/embed; SoundCloud tracks and sets incl. w.soundcloud.com?url=), and rebuilds the player URL against `FRAME_ORIGINS` (youtube.com, youtube-nocookie.com, consent.youtube.com, player.vimeo.com, open.spotify.com, w.soundcloud.com). Sizes: YouTube/Vimeo fit 400x225; Spotify 400 x 80/152/352; SoundCloud 400 x 166/400. Seam `FrameHost` (open/place/hide/close) + `ProviderFrames` (one live player); `StandInFrames` for tests and captures; `Players::set_host` swaps it.
- Desktop: `embed_frames.rs` hosts wry/WebView2 in a WS_CHILD container on the UI thread (clipped, not squashed). Isolation: own profile `<data dir>/embed-players`, InPrivate, browsing data cleared on close; the page is `kyoko://localhost/frame` (served as https://kyoko.localhost/) with CSP `default-src 'none'; frame-src <provider origins>`, strict-origin referrer, no script of its own, one sandboxed iframe (scripts, same-origin, popups, presentation; no top navigation). Top-level navigation only to https://kyoko.localhost/; other links and new windows open in the system browser (https only). Permissions denied, downloads refused, devtools, context menu, zoom, accelerators and clipboard off, no IPC. No web view exists before the click.
- Overlay rule: every 16 ms while a player lives, the box's bounds are hit-tested on a 6x4 grid in the zgx scene; anything else on top (menus, popouts, lightbox, dialogs, settings) hides the frame; partly visible = clipped; unmounted > 8 ticks = destroyed; sign-out closes it. Any new native child window must follow the same rule. Small popups that miss all 24 points can draw under the player.
- No WebView2 runtime: card says "Opens in your browser: WebView2 isn't installed", play goes through the link gate.
- New dep: `webview2-com` 0.39.1 (already in the lock via wry) for the runtime version check. Exe 118.9 MB release. Memory: idle 175 MB private; playing YouTube +~413 MB private across 8 WebView2 processes, freed when the player closes.
- Ids: `provider-play-`, `provider-open-`, `provider-frame-`, `provider-close-` + `embed-<message id>-<index>`. Mockcord unfurls the four providers ("showcase: providers").
- Deviations: kyoko adds a close button (Discord has none); one player at a time not checked against Discord; widget title clips instead of an ellipsis. Kyoko lacks Discord's help "?" button in the channel header (not built).
- Check on a real run: real Vimeo/Spotify/SoundCloud embed shapes, and whether YouTube accepts the https://kyoko.localhost referrer (error 153 otherwise; then the card can still open the browser).