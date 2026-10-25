# 26 — Message rendering

**What to build:** Messages render exactly as oxidcord renders them, with full Discord markdown and rich content.

**Blocked by:** 25 (Timeline view), 18 (Media pipeline), 17 (Fetch missing replies and users)

**Status:** resolved

- [x] Discord markdown renders in full: styles, headings, subtext, lists, quotes, code with highlighting, spoilers, masked links, mentions, custom emoji and timestamps
- [x] Embeds, attachments, stickers, polls, reactions, system messages, reply previews, edited markers and the unread divider all render
- [x] Animated emoji, GIFs and animated WebP play
- [x] Message text can be selected and copied
- [x] Parity captures match for a fixture that covers every kind of message

## Answer

Merged to main as `a75d905..ee133b3`. Docs: `docs/ui.md`, `docs/media.md`, `docs/parity.md`.

- Bridge: `bridge::Timeline` lowers each message once into `MessageEntry` (`crates/bridge/src/message.rs`): `body` (parsed markdown for content, embed descriptions and field values, cached per revision), `names` (mentions resolved through state; unresolved `@unknown-user`), `reply: ReplyPreview::{Message, Deleted, Unavailable, Loading}`, `system` (line and icon), `mine`. A resolved mention or reply arrives as `Delta.updated` for that row only.
- Desktop `apps/desktop/src/timeline/`: `rows.rs` (one renderer per row kind, reply header, system rows, deleted style: muted text with a red "(deleted)"), `body.rs` (markdown, one zgx `rich_text` per inline run plus blocks), `inline.rs` (custom emoji as a word-joiner placeholder with the image laid over its rect via `on_layout`), `attachments.rs`, `extras.rs` (reactions, polls with voting via `VotePoll`, thread chip), `pictures.rs` (gallery registry; `names_in(entry)` feeds `set_visible`), `links.rs` (masked-link gate: oxidcord's 10 trusted hosts plus `settings.trusted_link_hosts` open directly; others show a "Leave Kyoko?" zgx modal above the slot with "Always trust"), `icons.rs`. `timeline/avatars.rs` is gone. Unicode emoji are system-font text; no emoji set bundled; no new deps. Animated media plays only on screen.
- Mock safety: `bridge::local_media` maps every non-mock media URL to mockcord's `/media/<hash>.png` and `MediaFetcherBuilder::only_loopback` drops Discord hosts in mock runs, so fixtures never reach Discord.
- Fixture: mockcord `#showcase` channel in guild 0 (`tools/mockcord/src/fixtures/showcase.rs`), 8 sections opened by `-# showcase: …` markers; ids from the end of the generator. Oxidcord capture steps exported to `tools/parity/oxidcord-ref.patch`.
- Parity (whole window): timeline 7.6%, -media 5.8%, -markdown 7.1%, -inline 8.8%, -embeds 6.9%, -embeds-media 6.6%, -attachments 6.1%, -files 6.2%, -replies 7.1%, -system 7.2%; layout lines up. Remaining: text rendering, avatar/emoji art, panels of other tickets.
- Tests: 9 pure-logic tests and 1 seam test (reply to unloaded original + unseen-user mention resolve through backfill into `Delta.updated`) in `crates/bridge/tests/bridge/message.rs`. Memory: 239 MB steady under `--chatter 2` (debug).
- Known gaps: copying a custom emoji drops it (zgx strips the placeholder); forwarded messages show no snapshot (as oxidcord).
- For 27: pending rows 50% opacity, failed rows red; the failed notice with Retry is 27's. For 28: click handlers unwired for reaction chips (`reaction_key` builds `AddReaction`/`RemoveReaction`), mentions/avatars (profiles), reply previews (jump via `TimelineView::scroll_to(&RowKey)`, puts the row at the top) and the hover toolbar; hover is a per-row signal. 32: images have no click; use `attachment_request`/`embed_request` with `Quality::Original`, boxes from `bridge::fit_box`. 33: thread chips and ThreadCreated rows have no open action. 37: `chat.inline_media`, `chat.link_previews`, `chat.show_reactions`, `trusted_link_hosts` are read once at start via `timeline::Prefs`/`Links`. 48: names via `Names::of`/`person_name`. 52: deleted styling reads `MessageEntry.deleted`; with `ShowDeletedMessages` on, replies become `ReplyPreview::Deleted`.
