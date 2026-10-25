# 27 — Composer

**What to build:** The composer matches oxidcord and covers everyday writing: multiline text, IME, spellcheck, emoji and mention autocomplete, reply and edit, uploads, and slash commands.

**Blocked by:** 25 (Timeline view), 09 (REST safety layer)

**Status:** resolved

- [x] The composer matches oxidcord in the parity captures
- [x] Multiline input, IME and spellcheck work on Windows
- [x] Emoji and mention autocomplete, reply (with a ping toggle) and edit all work
- [x] Files can be attached by picker, paste and drag-and-drop, with previews
- [x] Slash commands run
- [x] A failed send shows its reason and is not retried blindly

## Answer

Merged to main as `d76296b..97b90c8`. Docs: `docs/ui.md`, `docs/rest.md`.

- Session: `SendMessage` gained `ping_reply` (off → web client's `allowed_mentions` with `replied_user: false`). New `RetryMessage`, `DiscardMessage`, `LoadApplicationCommands`, `RunApplicationCommand`. `task/outbox.rs`: sends and the web client's upload flow (`POST /channels/{id}/attachments` for slots, `PUT` to each slot on a separate client with browser headers only — no token, cookies or super properties — then the message with `uploaded_filename`); size/count limits checked before any request; last 16 failed sends kept for manual retry. `task/interactions.rs`: guild/channel/user command indexes, `POST /interactions` (multipart, `session_id`, nonce), paced like sends; `INTERACTION_FAILURE` reported. Failed edits raise `CommandRejected`. Typing: the composer sends `Typing` on every change, the session throttles to one post per 9 s, and a send resets it. State: `State::upload_limit`, `message_limit`, `command_index`, `Changes::commands`. A nonce'd send is still retried once on a short 429 (09's behaviour).
- Window: logic in `bridge::Composer` (no UI dependency), wired by `src/ui/composer.rs`; Luau `ui/shell/composer.luau` (oxidcord layout). Suggestions, notice bar, reply/edit bars and the file tray take layout space so the timeline slot shrinks; `timeline/mod.rs` `resize` keeps an at-bottom list pinned after the new height is laid out (2 tests in `timeline/scroll.rs`). DMs read "Message @Name".
- IME and spellcheck work through zgx (Windows IME composition; `spellcheck = true` uses the Windows spell checker). Drag-and-drop works (winit registers the OLE drop target; verified with a real drop); picker and Ctrl+V paste work; upload thumbnails go through `host.image`, not the gallery. Edit happens in the composer with an edit bar (not inline). Mentions are inserted as raw `<@id>` (as oxidcord). No slash option autocomplete or file-type options.
- Locks: the notice reads `State::channel_lock` on each relevant change and at `until`, shows `explanation()` as is, confirm sends `ConfirmRisky` then re-sends the held message; gates have no confirm button.
- Mockcord: attachments and upload routes, a Mockbot with `/ping`, `/echo`, `/roll`, `/broken`, `--captcha-once <pattern>`.
- Tests: 9 session-seam tests (`tests/session/composer.rs`), 8 bridge tests (`tests/bridge/composer.rs`, incl. empty-channel lock and screening gate). Parity: composer 7.7%, composer-reply 7.6% (whole window); composer field region 2.4–4.2%. No `composer-autocomplete` capture.
- Known flakes under full-suite load: `alerts::the_channel_in_view_stays_silent_until_the_window_loses_focus` (waits 300 ms), `capabilities::signing_out_after_a_refresh_leaves_no_token_behind`; both pass in isolation.
- For 28: host actions `composer.start_reply(message_id)` / `composer.start_edit(message_id)` (Luau `composer_start_reply` / `composer_start_edit`); Up-arrow edits the last own message. 32: GIF, sticker and emoji buttons are drawn without click handlers. 45/46/47: no plugin composer slot yet; TextReplace can use `before_send`/`before_edit`; SilentTyping can use `allow_typing`.
