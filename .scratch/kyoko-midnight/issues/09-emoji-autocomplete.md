# 09 — Emoji autocomplete

**What to build:** The `:` autocomplete attached above the composer in Midnight's style: "EMOJI MATCHING :sm", rows with the emoji, `:name:` and the server for custom ones, locked rows greyed with a lock, the selected row washed, the keyboard hint line. The same container serves `@` and `#` suggestions.

**Blocked by:** 03, 04

**Status:** resolved

**Mockups:** `docs/design/mockups/emoji-autocomplete.html`

**Spec:** `docs/design/midnight.md`, Expression picker

- [x] Matches the mockup
- [x] Locked emoji are listed but cannot be inserted
- [x] `@` and `#` suggestions share the container

## Answer

Merged to main as `e61eddc`. Docs: `docs/ui.md` (the suggestion container), `docs/parity.md`.

- Container: `popup` in `composer.luau`, still in the composer column (in flow, not an overlay: the timeline slot shrinks as before). Inset 8, `bg_3`, 1-point `border` on three sides, rounded top corners joined to the field; while open the typing line hides and the field and joined bars square their top corners (`suggesting()`). 24-point header ("EMOJI MATCHING" in `text_4`, `:sm` in `text_2`), 36-point rows (20-point emoji, `:name:`, server right-aligned in `text_5`), selected row washed with `hover`, hint line "↑↓ to navigate, ↵/tab to select, esc to dismiss". Midnight tokens only.
- Bridge: the picker's lock rule is the shared `emoji_lock`, so autocomplete and picker give the same reason; `Suggestion.lock`; `Composer::choose` refuses locked rows (Enter, Tab, click do nothing, the list stays); `Composer::heading` gives each trigger's header (`bridge::suggestion_heading`). Emoji show up to 9 rows with up to 2 kept for locked emoji; other triggers 8. Locked emoji are left out of `RankEmoji` candidates.
- Desktop publishes `composer.heading` (`{title, query}`) and each row's `lock` for both composers. When the field echoes kyoko's own replacement it now uses the caret kyoko set (a programmatic draft never opened suggestions before).
- Mockcord `--autocomplete-emoji` adds `smug`, `smol_wave` (first guild), `smolcat`, animated `smash` (second guild), ids allocated last.
- Tests: bridge `composer::locked_emoji_are_listed_last_and_never_inserted` (no-Nitro mockcord; also the Members and Channels headings); load check `ui::scripts::emoji_members_and_channels_share_the_suggestion_container`. Capture `emoji-autocomplete` (`ComposerShot::EmojiAutocomplete`, `--no-nitro --autocomplete-emoji`), mapped in `MOCKUPS`.
- Checks re-run by the orchestrator in lane A on top of M20: all green. Mockup parity (port 8494): emoji-autocomplete 11.9%.
- Deviations: custom emoji rank before Unicode (existing ranking; the mockup lists Unicode first); the lock reason is a tooltip on the lock icon, not the row.
- For M10: the panel sits at the top of the composer column; anything taking that spot reuses `suggesting()`. `/` suggestions already render here with a "Commands matching" header; M10 replaces that branch. If the command picker must reach 420 points over the timeline, either keep shrinking the slot or move it to Rust above the slot. Row ids `<area>_suggestion_<n>`, panel id `<area>_suggestions`.
