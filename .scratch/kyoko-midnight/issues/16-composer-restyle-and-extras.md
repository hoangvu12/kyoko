# 16 — Composer restyle and extras

**What to build:** Restyle the composer to Midnight (47 high, `--bg-3`, 1-point `--hover` border) with `+` and GIF, sticker, emoji and apps buttons (no gift, no send). Add Discord's live markdown inside the field (bold, italic, inline code, spoiler, quote, mentions and channels as chips, custom emoji as images, markers dimmed), the grow-to-half-the-chat rule, the reply bar with the @ toggle, the character counter (under 200 left, red past the limit), the upload tray (216 cards, spoiler, edit and remove on hover), the drop overlay ("Upload to #channel", Shift uploads directly), upload progress and failure (Retry, Remove), the too-big notice, and the `+` menu (Upload a File, Create Thread, Create Poll, Use Apps).

**Blocked by:** 01, 03; v1 27

**Status:** resolved

**Mockups:** `docs/design/mockups/composer-markdown.html`, `docs/design/mockups/composer-markdown-over-limit.html`, `docs/design/mockups/composer-uploads.html`, `docs/design/mockups/composer-drop.html`, `docs/design/mockups/composer-upload-failed.html`, `docs/design/mockups/composer-plus-menu.html`

**Spec:** `docs/design/midnight.md`, Chat panel, Composer extras, Composer buttons

- [x] Each state matches its mockup
- [ ] Markdown in the field does not change the sent text; IME and spellcheck still work
- [x] Upload progress and spoilered uploads are covered by session-seam tests
- [x] Shift on drop uploads directly

## Answer

Merged to main as `040077f`. Docs: `docs/ui.md` (composer, "Live markdown in the field"), `docs/parity.md`.

- Composer: 47-high field inset 8 from the chat panel's sides and bottom (top lines up with the user panel), `bg_3` with a 1-point `border_light` border, typing line above it. Notice, reply (`@ ON`/`@ OFF`, round close), edit and upload tray share one block joined above the field (`joined_bars`). `+` left; right: the `composer.buttons` plugin slot, then GIF, sticker, emoji (ids `composer_gif_button` etc., M08's picker action and anchor kept). Counter at bottom right under 200 left, `red_2` past the limit; the field grows a line at a time to half the chat column, then scrolls. "Waiting for connection..." while disconnected.
- `+` menu: a `kit` menu 8 above the field with Upload a File and Create Thread (when allowed), in a new root layer `composer_layer.rs` (in `LAYERS`) with the attachment edit dialog and the drop card.
- Uploads: 216 cards on `bg_4` with a spoiler/edit/remove toolbar on hover; spoilered cards blurred under a SPOILER pill; Edit opens a dialog (file name, alt text, Mark as spoiler). A spoilered file goes out as `SPOILER_<name>` with alt text as `description`. Uploads stream in 64 KiB chunks with progress under the pending message (`bg_2` track, `accent_2` fill, cancel aborts); a failure shows "Upload failed · Retry · Remove"; the too-big notice names the file, its size and the limit. Drop: dashed "Upload to #channel" card over a dimmed window; Shift on drop (read with `GetAsyncKeyState`, winit passes no modifiers with an OLE drop) sends the files at once as their own message, leaving draft, reply and tray alone.
- Mockcord: `hold_requests`/`release_requests`; `stalled-upload.mp4` never answers, `broken-upload.mp4` gets a 500 (for captures).
- Live markdown: partial, by user decision (2026-10-26, "accept partial for now"). zgx `decorations` give flat backgrounds behind byte ranges: code `bg_4`, spoilers `bg_2`, raw mentions/channels `mention_background`; the bridge already finds every marker position. Not done: dimmed markers, bold/italic faces, chips with names, the quote bar, emoji images (mentions show as raw `<@id>`). zgx's `highlight` is broken in a wrapped editable field (caret three lines low, a ~4,000-character line not drawn, placeholder gone). zgx would need: a highlighter matching the wrapped layout; per-range weight/italic/family in the editor layout; padded, rounded range backgrounds and a line decoration; atomic inline objects over raw text; all exposed on `ui.textarea` and single-line inputs. Revisit when zgx gains them; no upstream report for now. Sent text equals what was typed (`markdown_in_the_field_is_marked_but_sent_exactly_as_typed`); the field keeps `spellcheck`; IME composition not exercised.
- Tests: session `upload_progress_shows_on_the_pending_message_until_it_is_sent`, `cancelling_an_upload_in_flight_stops_it_and_posts_nothing`, `a_failed_upload_keeps_its_files_and_a_retry_uploads_them_again`, `a_spoilered_upload_reaches_discord_as_a_spoiler_with_its_alt_text`; bridge `shift_on_drop_uploads_the_files_directly_and_leaves_the_draft`, `a_spoilered_and_edited_file_is_sent_as_edited` (the Shift read itself untested). Checks re-run by the orchestrator in lane A: all green.
- Mockup parity (port 8494): composer 14.0%, composer-reply 13.9%, composer-markdown-over-limit 13.7%, composer-uploads 18.4%, composer-drop 11.2%, composer-upload-failed 13.1%, composer-plus-menu 12.8% (five newly mapped); emoji-picker 20.7% after the composer moved.
- Deviations: no apps button, no Create Poll / Use Apps (M15, M17); typing names not semibold; the too-big notice shows the mock account's Nitro limit; Shift-drop sends without a reply reference; the `+` menu's first item shows zgx's focus ring when it opens.
- Capture constants: `COMPOSER_PICKER` (954, 755), `SILENT_TYPING_BUTTON` (838, 744).
- For later tickets: M09: the suggestions popup is still oxidcord's, inset 8 above the typing line (`popup` in `composer.luau`); `bridge::text_marks` gives mention and emoji ranges. M10: anchor the command picker to the `composer` slot; the 36-high option bar goes in `joined_bars`. M15: the apps button after emoji in `toolbar()`, "Use Apps" in `composer_layer.rs` (`PlusMenu`, `Request`). M17: "Create Poll" in the same menu.
