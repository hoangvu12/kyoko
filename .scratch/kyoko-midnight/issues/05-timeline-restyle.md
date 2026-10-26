# 05 — Restyle the timeline

**What to build:** Restyle the message list to Midnight: rounded message cards with `--message-hover`, the mention and reply-to-you gradients with their 4-point bars, the unread divider and the floating new-messages bar, embeds (radius, 4-point colour bar), code blocks on `--bg-3`, attachments, the hover bar, deleted and edited styling. Add what the mockups show around expressions: GIFs from the GIF provider shown as the video alone (no embed card) with the favourite star on hover and a GIF pill when autoplay is off, stickers at 160, jumbo emoji at 48, the emoji popout (custom: server and Join; Unicode: "A default emoji...") and the sticker popout, placed beside what was clicked and kept inside the chat. A sent link's embed can be removed by its author (Remove all embeds, the `SUPPRESS_EMBEDS` flag).

**Blocked by:** 01, 03, 04; v1 25, 26, 28

**Status:** resolved

**Mockups:** `docs/design/mockups/midnight-discord.html`, `docs/design/mockups/expressions-in-messages.html`, `docs/design/mockups/expressions-in-messages-emoji-popout.html`, `docs/design/mockups/expressions-in-messages-sticker-popout.html`, `docs/design/mockups/link-preview-while-typing.html`

**Spec:** `docs/design/midnight.md`, Chat panel, Expressions and commands

- [x] The timeline matches its mockups in captures (02)
- [x] GIF favourites toggle from the timeline and appear in the GIF picker
- [x] Removing embeds is a session command with a behaviour test against mockcord
- [x] Popouts never cover the item they opened from and stay inside the chat

## Answer

Merged to main as `29059ed..2fbe224`. Docs: `docs/ui.md` (Midnight timeline, GIF Favorites tile), `docs/rest.md` (favourite GIFs, removing embeds), `docs/parity.md`, `THIRD_PARTY_NOTICES.md` (two new icons).

- Rows: rounded cards at the mockup's positions with `message_hover`; mentions of you and replies to your messages get Midnight's fading wash and a 4-point bar (`MessageEntry::highlight`, `Highlight`). Author line, timestamps, reply preview, bot tag, NEW divider, code blocks, embeds, file cards and the hover bar restyled. Deleted and edited styling lives in 52's `timeline/kept.rs` on Midnight colours (ids unchanged).
- New-messages bar: floating, inset 12 in the chat panel, 6 below the slot's top ("N new messages since …" jumps to the divider; Mark As Read sends `MarkChannelRead`).
- GIFs: Klipy/Tenor GIFs show alone, at most 400×300; a link-only message hides the link text. Hover shows the favourite star (filled if favourited); with autoplay off, dimmed with a GIF pill until hovered. Favourite GIFs live in the frecency settings proto (field 2) beside favourite emoji, read before writing so favourites made elsewhere stay; the GIF picker has a Favorites tile and opening it sends `LoadFavorites`.
- Popouts (`timeline/popout.rs`, placement rule in the bridge): custom emoji, Unicode emoji (also jumbo) and stickers open a popout beside the item, never covering it and always inside the chat; outside press in the timeline, Escape or a channel switch closes it.
- Remove all embeds: `Command::SuppressEmbeds` sends only `flags` with SUPPRESS_EMBEDS, own messages only, offered in the hover bar on own messages that show embeds (not in the context menu).
- Mockcord: posted links unfurl to a link or GIF (`gifv`) embed; favourite GIFs stored and served; a flags-only edit no longer marks a message edited.
- Tests: `a_favourited_gif_is_read_then_written_to_the_frecency_proto_and_unfavouriting_removes_it` (session), `a_gif_favourited_from_a_message_appears_in_the_pickers_favourites` (bridge), `embeds::removing_embeds_sets_the_suppress_flag_on_your_own_message_only`, `popouts::a_popout_never_covers_its_item_and_stays_inside_the_chat`. Checks re-run by the orchestrator in lane A after rebasing onto 43 (unions in `capture.rs` and `docs/parity.md`): all green (`xtask test` hit an sccache dropout once, green on rerun).
- Mockup parity (port 8494): timeline/main screen 17.6%, expressions-in-messages 27.5%, -emoji-popout 26.6%, -sticker-popout 26.4%, link-preview-while-typing 21.1%; by eye positions, sizes and colours match (placeholder art and authors differ). New capture screens for the four expression and link pages are mapped in `MOCKUPS`.
- Deviations: no Join Server button (kyoko can't name a server you're not in without Discord's emoji-source endpoint; the popout says so); the GIF pill and star can't share a screen (autoplay is global); the divider adds 10 points below it (a zero-height row was clipped); favourite GIF tiles stored as MP4 show only a colour; popouts close only on presses inside the timeline; mention chips have square corners.
- For later tickets: M06: reaction chips in `extras.rs` are still the old shape. M07: the hover bar is `MessageActions::toolbar` with ids `message-action-<label>`; the popout is `timeline/popout.rs`. M16: the new-messages bar follows the slot if the composer changes height. M22: the thread chip in `extras.rs` isn't restyled; the thread pane should reuse `Highlight` and the row frame in `rows.rs`. Capture helpers `TimelineView::popout_anchor`, `open_popout`, `newest_gif`; `capture::LAST_MESSAGE` (640,690) still lands on the newest message.
