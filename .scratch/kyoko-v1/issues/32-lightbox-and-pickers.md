# 32 — Lightbox and pickers

**What to build:** The image lightbox and the emoji, GIF and sticker pickers match oxidcord.

**Blocked by:** 26 (Message rendering)

**Status:** resolved

- [x] The lightbox and pickers match oxidcord in the parity captures
- [x] The lightbox shows images at full resolution
- [x] Picking an emoji, GIF or sticker inserts or sends it

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `274f304..413a376` (rebased over 45 by the orchestrator; union conflicts in `ui.rs` and `docs/ui.md`). Docs: `docs/ui.md` ("Expression pickers", "Lightbox", `composer.picker` action), `docs/rest.md` ("GIFs and stickers"), `docs/media.md`, `docs/parity.md`, `docs/themes.md`.

- Lightbox (`apps/desktop/src/timeline/lightbox.rs`), drawn in Rust above the timeline slot: opens from image attachments, bare image embeds and large non-video embed images; Escape, Close or a backdrop click closes (a click on the image doesn't); Open original button and file name; image at most 88% of the window. Fetches the original (`Quality::Original`, no proxy width or format) fitted to the window: bridge test `media::the_lightbox_shows_the_original_attachment_fitted_to_the_window`.
- Pickers (`apps/desktop/src/pickers/`: `emoji.rs`, `gif.rs`, `sticker.rs`; window side `src/ui/expressions.rs`): oxidcord's three-tab picker with search, above the clicked composer button. 28's reaction picker and 30's custom-status picker use the same component (moved from `timeline/picker.rs`). Models in bridge: `EmojiPicker` (`rail()`, `section()`, `default_section()`, `PickerSection::Recent` first when recents exist), `StickerPicker`/`StickerCell`, GIF search.
- Sends: emoji inserted at the caret (`<:name:id>` / `<a:name:id>` for custom); a GIF sends its link; a sticker sends `sticker_ids`; both through the existing outbox (locks, nonce, pending row, pacing, failure and retry; a failed sticker retries as a sticker). In a locked channel the pick waits for "I understand" and the draft is untouched. Unusable emoji and stickers (other servers' without Nitro) are dimmed and inert.
- GIFs: the web client's shapes (`/gifs/trending`, `/gifs/search` with `media_format=mp4`, `provider=tenor`, `locale`; `/gifs/suggest` with `limit=5`); search after 300 ms of quiet, at once on Enter, a suggestion chip or a category. `POST /gifs/select` only when the user turned GIF-usage sharing on (37's setting). Tiles use the still `preview` and load `gif_src` on hover, playing as the animation setting allows. On real Discord, category images arrive as `.mp4` (undecodable), so category tiles show colour and name only.
- Stickers: guild stickers now come from READY and GUILD_CREATE (`State::guild_stickers`; mockcord READY carries them); sticker packs load once when the sticker tab first shows. The rail lists every server with stickers (oxidcord: current server plus five).
- Theme tokens `brand_selected`, `gif_shade`.
- Tests: session seam `tests/session/expressions.rs` (GIF search shapes, GIF send through the outbox, sticker send, failed sticker retry); bridge tests (emoji insertion incl. caret, multi-byte, custom markup, locked refused; GIF debounce; sticker picker and Nitro gating; sticker under a lock). Driven in the real app on mock via `Ui::dispatch`. `headless-run` reports `expressions_ok: true`.
- Parity (port 8496, 150% scale): lightbox 2.6%, emoji-picker 7.4%, gif-picker 7.5%, sticker-picker 7.6%; re-checked by the orchestrator after the rebase. Remaining: mock artwork, text rendering, oxidcord's focused search ring.
- Gotchas for overlay work: a popover in the root `overlay()` must sit inside a zero-size absolute box, or it swallows every click on the window; a scroll area or wrap row whose content arrives after a popover opens keeps its first (empty) size, so give panes explicit sizes.
- For 50: reorder in `bridge::EmojiPicker` or `State::emoji_catalog` (through 45's `hook:emoji_rank`, off the UI thread). For 53: `StickerPicker`/`StickerCell` carry the full `Sticker` with lock state; emoji cells carry `custom` (id, animated) and `markup`; the session has `CreateSticker` and `EditEmoji`.
