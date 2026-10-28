# 90 — New logo from the user's image

**What to build:** The user picked a new logo: `https://cdn.donmai.us/sample/f4/ec/__hyakuto_kyoko_hololive_drawn_by_un1rock__sample-f4ec8fcadfbcc84bb2636459256be1ec.jpg` (a banner; crop on her face). Fan art by un1rock; the user chose it.

**Blocked by:** none

**Status:** resolved

- [x] Download it into its own new folder outside the repo (untrusted file; neutral User-Agent `kyoko-dev`); crop a square on her face; produce the sizes kyoko needs (exe `.ico` 16/24/32/48/64/256, tray, window, the login and loading marks where `kyoko_mark` is used, README if it shows a logo)
- [x] Replace the old logo everywhere it's used; the app icon in the taskbar and tray shows the new one; captures of login and the window title checked by eye
- [x] Source URL and artist credited in `THIRD_PARTY_NOTICES.md` or `README.md`

## Answer

Merged to main as `5975798` (worker `cb98b3c3`, lane A; full verify re-run by the orchestrator: all green; `parity mockup` login-web 5.6%, server-rail 17.6%, checked by eye). Docs: `docs/ui.md`, `packaging/README.md`, `THIRD_PARTY_NOTICES.md` ("Kyoko's icon": source URL, artist un1rock, character Hyakuto Kyoko of hololive / COVER Corporation, not covered by kyoko's licences), README credit line.

- Source downloaded once with `kyoko-dev` into `nguyenvu/.cache/logo-90/` (850x478 JPEG), processed with `python -I` from `nguyenvu/.cache/logo-tools/` (Pillow venv `.cache/logo-tools-venv`); `generate.py` there rebuilds every asset.
- Crops (left, top, right, bottom in source px): wide `(385, 20, 785, 420)` for 48/64/128/256; mid `(420, 70, 740, 390)` for 24/32; tight `(450, 120, 710, 380)` for 16 and 16-point UI spots.
- Assets: `assets/icon.ico` (16/24/32/48/64/256, embedded by `build.rs`, so exe, window and taskbar follow), `assets/icon.png` (256, README and toasts), `assets/icon_32.png` (tray, `tray.rs` one line), `apps/desktop/ui/images/kyoko.png` (128) and `kyoko_small.png` (48). Removed `assets/icon.svg`, `ui/icons/kyoko.svg`, `ui/icons/kyoko_mark.svg`.
- UI: `components/kyoko_mark.luau` (`{ size, rounded? }`, small crop at 20 points and under) in the web-login title bar, the QR code centre (40 points, `radius_sm`), and the rail's home tile (fills it like a server icon; hover spin and shrink kept). Exe icon extracted and window icons read with `WM_GETICON` (48 and 24 px show the new face); the tray wasn't looked at (no OS input on the shared desktop).
- Deviation: Midnight's home tile is a tinted glyph; a picture can't be tinted, so it doesn't change colour on hover or selection. `icon.ico` grew 20 KB -> 137 KB.
- For later: Luau loads local pictures from `apps/desktop/ui/images/` by relative path; release packaging (58) must ship that folder with `ui/`.
