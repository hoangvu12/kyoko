# 25 — Timeline view

**What to build:** The message timeline is a Rust view next to the Luau UI. It follows new messages while the user is at the bottom, keeps its place when older history loads above, and loads earlier pages as the user scrolls up.

**Blocked by:** 20 (Theme, fonts, icons and app frame)

**Status:** resolved

- [x] The timeline uses measured rows keyed by message id
- [x] It stays at the end while the user is at the bottom
- [x] When older history is added above, the first visible row stays in place
- [x] Scrolling up loads older pages automatically
- [x] Plain text messages render in oxidcord's grouped layout and match the parity captures

## Comments

From the spike (04): replace the spike's `bridge::Timeline` (whole-list projection, plain text) with the Rust timeline view; publish changes instead of the whole list; show local timestamps. Posted messages reached the UI in a median of 1.4 ms with none dropped or duplicated. Ack behaviour belongs to the session (ticket 10): send `SetViewport` and `SetViewing`, never per-message acks.

## Answer

Merged to main as `dfdad0a..caf0ec6`. Docs: `docs/ui.md` (timeline and overlay rule), `docs/parity.md`, `docs/media.md`.

- `apps/desktop/src/timeline/`: zgx `measured_virtual_list` keyed by `RowKey::{Loading, Start, Day(date), Unread, Message(id), Pending(nonce)}`; heights kept by key, unmeasured rows estimated (`rows::estimate`); one value signal per key so an edit redraws one row. `VariableHeights::anchor_end()` follows the end; a reset scrolls to the end.
- `scroll::Anchor` records the first visible rows by key and offset when not at the bottom and restores the first survivor after any change (prepends and removals above). `scroll::Paging`: `LoadOlder` within 14 rows of the top, `LoadNewer` when detached and at the bottom, once per edge message, retried after 3 s. `scroll::Reporter`: `SetViewport { first, last, at_bottom }` (message ids, ≤ every 250 ms, on change; at bottom = within 2 pt). No acks, no `SetViewing` (40 owns it). 8 pure-logic tests in `scroll.rs`.
- Rows: one render function per row kind in `rows::render`; grouped headers and follow-ups like oxidcord; day dividers; local time via `bridge::clock` (`message_time`, `clock_text`, `day_label`, `full_time`); system messages as muted text (no icons yet). New theme token `message_hover`. Text is selectable with Ctrl+C copy (zgx `selectable`).
- Avatars via the gallery (`Circle`); visible rows ±2 merged with the user-panel avatar and Luau `media.visible` into one `set_visible`. Default avatars fall back to `<cdn>/embed/avatars/<n>.png` (`media::cdn::default_avatar_url` takes the CDN base; mockcord serves it).
- Bridge `Timeline`: `select`, `close`, `apply`, `take_delta() -> Option<Delta { reset, prepended, appended, inserted, updated, removed }>`, `rows()`, `messages()`, `has_older()`, `is_detached()`; `MessageEntry` carries display fields, `Arc<Message>` and `rendered`. The spike's `timeline.messages` shared value is gone. 7 bridge tests.
- Live run: appends follow at the bottom, the view holds still when scrolled up, older pages load with the first visible row fixed, scrolling back re-pins and the session shrinks the window.
- Parity: message area 11.46% → 10.52% differing; message positions match oxidcord exactly. Remaining: embeds, reactions, mention pills (26), oxidcord's late member data in the capture, avatar art, ClearType vs greyscale, zgx scrollbar not drawn at rest.
- Overlay rule: the Rust timeline sits above the whole Luau tree, so Luau menus/popovers/lightbox over the message area render underneath it; such tickets must draw the overlay in Rust above the slot or hide the slot while it is open (recorded on 28, 30–33, 35–38).
- For 26: extend `rows::render`; deleted style, reply preview (`reply_target`), attachments (`attachment_request`), system icons still to do. For 27: `Pending` rows at 50% opacity, failed in red. For 28: hover state is a per-row signal; jump-to-message needs a scroll-to-key helper (`LoadAround` arrives as a reset).
