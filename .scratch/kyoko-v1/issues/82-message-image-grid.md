# 82 — Multiple image attachments: Discord's grid

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). A message with two images of different shapes renders them cropped side by side in a short strip (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_1.png`); Discord shows the same message as a mosaic that keeps both images readable (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_2.png`). Match Discord's attachment grid for 1 to 10 images (sizes, crops, gaps, radius).

**Blocked by:** none

**Status:** resolved

- [x] Layout rules for 1 to 10 images written in `docs/` (from observation and public sources)
- [x] The timeline lays out multi-image messages that way; single images keep their aspect ratio within the max box
- [x] Mock data with 2, 3, 4 and mixed-shape images; capture checked by eye against the Midnight mockup if one exists

## Answer

Merged to main as `015818f` (worker `fb0181ca`, lane C; rebased onto `795e2b0`, full verify re-run by the orchestrator: all green; kyoko-only captures `timeline-attachments`, `timeline-image-grid`, `-three`, `-four`, `-ten` checked by eye). Docs: `docs/ui.md` ("Image grid"), `docs/media.md`, `docs/parity.md`.

- Root cause: each image attachment was drawn alone, fitted to its own box; nothing laid out a message's images together.
- `bridge::mosaic` places tiles: grid 550 wide, 4-point gap, cropped tiles with `radius_md`. 1 image keeps its shape in 550x350; 2 are 273-point squares side by side, or stacked full width (max 173 high) when both are at least 2:1 wide; 3 is a 365 square left plus two 181 squares right; 4 is 2x2; 5+ are rows of three with the leftover in the first row (5 = 2+3, 7 = 1+3+3, 10 = 1+3+3+3). Tiles are placed in percentages with a fixed aspect ratio, so the grid shrinks with a narrow column like Discord's `min(550px, 100%)`.
- `attachments::image_grid` draws it where the first image was; file and audio cards follow. Each tile opens the lightbox; GIF tiles keep the star and GIF pill. `bridge::cover_box` sizes each request to cover its tile; `Pictures::attachment` takes the tile size; the row-height estimate assumes the full 550 grid.
- Mockcord: `Server::post_images` (new `sizes` on `Draft`), showcase `IMAGE_GRID_SECTIONS` at the start of `#showcase`.
- Tests: `crates/bridge/tests/bridge/mosaic.rs` (layouts for 2 to 10 and a single image; requests cover their tiles).
- Guesses, not observed: the 2:1 stacking threshold and 173 cap, the full-width leftover tile for 7 and 10, the 10-image split; gaps shrink with the grid (Discord keeps 4 px); message text stays above the images.
- For later: 89 can step through a message's images with `bridge::mosaic_images(&message.attachments)` and `bridge::LightboxImage::attachment`; 93 can use `timeline-image-grid-ten` (10 requests at once) as its load case.
