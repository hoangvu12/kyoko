# 105 — Screen capture and Go Live picker (local only)

**What to build:** Port WGC screen and window capture and the picker dialog with thumbnails, resolution and fps. Local preview only; no network.

Plan ticket V9 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 104

**Status:** resolved

- [x] The picker lists screens and windows; picking one previews it; closing the window ends capture
- [x] UI test for the picker; captures checked by eye

## Answer

Merged `fc53d7d` + `46e7132` (lane B, worker `e02b4654`), verified by the orchestrator: fmt, clippy, build, test --workspace, xtask test, headless-run, deny, luau0-src 0.22.0+luau740 all green; tls-capture against chrome-156 0/0 differences; kyoko-only captures `voice-go-live`, `voice-go-live-preview`, `voice-call` checked by eye (no Go Live mockup exists).

- Capture lives in `crates/video/src/screen.rs` (ported from oxidcord WGC): `screens()` lists monitors ("Screen 1", primary first) and shareable windows; `open_screen(target, VideoQuality)` returns a `VideoSource` beside 104's camera, scaled and converted BGRA to NV12 (`bgra_to_nv12` in `scale.rs`); `screen_thumbnail` takes one frame. A closed window gives `VideoError::Ended`; `CameraPreview::start_until` stops on it and runs an end callback (toast "The window you picked was closed.", back to the grid).
- Host `apps/desktop/src/ui/go_live.rs` owns the picker state through shared values `voice.screens`, `voice.screen_preview` and actions `voice.go_live`, `voice.screen_preview`. Under `--mock` it lists four fixed sources and uses the test pattern: tests and captures never capture the real screen. `crates/video/examples/share_window.rs` captures one window by PID for hand runs.
- UI `shell/go_live_picker.luau`: Applications / Screens tabs, 3-column thumbnail grid, picked step with 16:9 preview, real size and rate, Resolution 720p/1080p/Source, Frame Rate 15/30/60, Change, Cancel, Go Live (disabled until 107). In-process test `ui::scripts::voice::the_go_live_picker_lists_screens_and_windows_previews_one_and_closes`.
- Not done: Nitro gating of resolution/fps, app icons on window tiles, a "share audio" option. Thumbnails of real windows briefly start a WGC session each; the yellow border is off where Windows allows.
- For 107: feed `open_screen(target, VideoQuality)` into 104's `video-send` path (frames are already NV12 at the picked size and fps); enable `go_live_start` and send op 18.
- zgx notes: a closed `ui.modal` keeps its children mounted, so wrap content in `ui.show(open)`; `ui.scroll` keeps a stale height when its content swaps, so the picker doesn't scroll and shrinks the preview on short windows; when a step rebuild removes the focused node, focus a node in the new step on mount or Escape stops closing the modal.
