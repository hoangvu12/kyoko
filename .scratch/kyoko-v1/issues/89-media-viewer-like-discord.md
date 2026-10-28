# 89 — Image viewer like Discord's

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Clicking an image in Discord opens a viewer (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_3.png`): the image large and centred over a dark scrim, the author and time at the top left, and a toolbar at the top right with zoom, forward, download (save), open in browser, more (copy image, copy link), and close; zoom with the wheel and drag to pan; arrows between the message's images. Copy that UI.

**Blocked by:** none

**Status:** resolved

- [x] Viewer opens from attachments and embed images, full window over a scrim, Esc and click outside close
- [x] Toolbar: zoom in/out (and wheel zoom, drag to pan, double-click to reset), forward (opens kyoko's forward dialog), save (Windows save dialog, original file), open in browser, copy image, copy link, close; left/right between images of the same message
- [x] Author avatar, name and time at the top left
- [x] Capture checked by eye; in-process UI test for open, zoom, next and close

## Answer

Merged to main as `4aec041` and `55999f9` (worker `6709db5b`, lane C; full verify re-run by the orchestrator: all green; kyoko-only capture `lightbox` checked by eye against b_Image_3.png; `parity mockup lightbox` 52.2% because Midnight's `lightbox.html` still shows the old file-name toolbar, Midnight 07 measured 50.9%). Docs: `docs/ui.md` ("Lightbox").

- Root cause: ticket 32's lightbox was oxidcord's (file name, Open original, Close) and knew only the clicked image, not its message: no author, siblings, zoom or Discord's actions.
- Viewer: image centred on a 90% black scrim; 40-point avatar, name and time top left (`bridge::message_time`); toolbar group on `bg_3` with Zoom (toggle fit/2x), Forward, Download, Open in Browser, More (Copy Image, Copy Link), Close in its own group. Wheel zoom about the pointer 1x-8x, drag to pan (clamped), double click resets. Arrows and arrow keys step through the message's images in grid order and wrap; the root takes arrow keys in the capture phase while open. Escape, Close or a click outside the image close it.
- Opens from grid tiles, GIF tiles, bare image and gifv embeds, large embed images, and Components V2 gallery tiles (as a lone image). Embeds come after attachments when stepping.
- Actions: Forward closes and calls `MessageActions::forward` (the message menu's hook). Download uses zgx `Platform::save_file`, then `Pictures::save_original` through the media service (disk cache first), toast with the result. Open in Browser via `Links::open` (Leave Kyoko? check for untrusted hosts). Copy Image via `platform::clipboard::write_image` (`Pictures::pixels`).
- API: bridge `Viewing::of(entry, image)`/`Viewing::alone`, `message_images`, `Zoom`, `viewer_file_name`; `Lightbox::show(Viewing)`; `TimelineView::viewer_key`. Ids `viewer`, `viewer-image`, `viewer-author`, `viewer-zoom`, `viewer-forward`, `viewer-save`, `viewer-open`, `viewer-more`, `viewer-copy-image`, `viewer-copy-link`, `viewer-close`, `viewer-previous`, `viewer-next`; grid tiles `image-tile-<attachment id>`.
- Tests: `timeline::viewer_tests::the_viewer_opens_on_a_tile_zooms_steps_and_closes` (in-process, real session and mockcord), `crates/bridge/tests/bridge/viewer.rs` (order, author, wrap, zoom maths).
- Deviations: time in kyoko's message format, not Discord's bare clock; one zoom toggle; wrap at the ends (unobserved); embeds included even when link previews are hidden.
- For later: zgx replaces a node's first `reactive_style` with a second (keep position and bitmap on separate nodes); `TestApp` counts double clicks only through `dispatch_with_modifiers`. 91 can host video frames in `viewer-image`. The `docs/parity.md` ticket cell must be digits only.
