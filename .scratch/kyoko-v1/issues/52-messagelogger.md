# 52 — MessageLogger

**What to build:** Deleted messages and edit history show inline, styled like Vencord's MessageLogger, using kyoko's retention.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 16 (Keep deleted and edited messages), 26 (Message rendering)

**Status:** resolved

- [x] The message render hook can access kept deleted and edited versions
- [x] Deleted messages show inline in a deleted style, and edit history can be viewed
- [x] Settings control what is shown, and the retention limits apply

## Answer

Merged to main as `fb5b42d`. Docs: `docs/plugins.md`, `docs/storage.md`, `docs/ui.md`, `docs/testing.md`, `docs/parity.md` ("52 MessageLogger").

- Hook `render_message` (`hook:message_render`): asked only about messages with something kept (deleted, or at least one earlier version), with `{ id, channel_id, guild_id?, author_id, bot, mine, content, deleted, deleted_at?, edited_at?, versions }`; returns `{ deleted = "text"|"tint"|"hide", history = "inline"|"collapsed", accessory = text | { text, action? } }`. 50 ms deadline (`Limits::render_deadline`), chained in plugin id order, fails open to kyoko's look with a strike. `show_deleted()` (same permission) is asked on every plugin change and session attach; the host sends `Command::ShowDeletedMessages(true)` while any running plugin answers true, false once none does.
- Cache: `bridge::Display` gained `Formatting::Messages` with its own memo (2048 entries, keyed on id, deleted flag, version count, edit time, oldest kept version); answers come through the `display-hooks` thread and the wake in `ui/display.rs` redraws only changed rows (`Display::look_calls()`). `MessageEntry` gained `look` and `history` (earlier versions parsed once per versions `Arc`, mentions resolved); a `"hide"` row is left out and the next message by the same author gets its header back. Any plugin action now bumps that plugin's revision, so its cached answers are re-asked.
- Drawing in `apps/desktop/src/timeline/kept.rs` (`text_color`, `trailer`, `tinted`, `history`, `history_toggle`, `accessories`): red text or a `background_feedback_critical` tint, earlier versions dimmed above the current text, "(edited)"/"(deleted)" opens a collapsed history, accessory lines under the message; `rows.rs` only calls in, `message_frame` applies the tint; body `Look` gained `on_trailer`. The `ui:message.accessory` slot is real: clicks go `PluginMenu::on_accessory` → `Link::click_accessory` → `PluginHost::gesture`.
- MessageLogger (`plugins/message-logger/`, bundled, ships disabled; `hook:message_render`, `ui:message.accessory`, `storage`): show both / deleted only / edits only, red text or red tint, inline history, ignore bots, ignore my messages, "Remove from log" (plugin storage, at most 500 ids). Retention is kyoko's (16); the plugin can't keep more, as its setting text and the docs say. Left out: Vencord's per-user/channel/server ignore lists; earlier versions show text only.
- Mockcord: a client message containing "spoiler alert" (`mockcord::MODERATED_PHRASE`) is deleted 400 ms later, so captures can show a deleted message (kyoko never keeps its own deletes).
- Tests: 5 plugin-seam (`crates/plugins/tests/plugins/message_logger.rs`), 2 bridge tests with the real host and mockcord (exactly 2 plugin calls, none on 50 redraws), `timeline::kept_tests` (`TestApp`, real clicks), `headless-run` reports `kept_calls: 2`. Checks re-run by the orchestrator in lane C after rebasing onto 42 (a mechanical union in `capture.rs`): all green.
- Parity (port 8493, kyoko only, `KYOKO_CAPTURE_PLUGINS=message-logger`): new `message-logger-deleted` and `message-logger-edits`, checked by eye on the Midnight shell. Plugin off, nothing in the timeline changes.
- Known gap: a hidden deleted message can show for one frame before the `"hide"` answer arrives.
- For later tickets: M05 restyles kyoko's own deleted/edited styling in `timeline/kept.rs`; keep the ids `message-text-<id>`, `kept-version-<id>-<n>`, `accessory-<plugin>-<id>`. Another plugin's accessory: return `accessory = { text, action }`; the click arrives as `on_action(action, { source = "slot", slot = "message.accessory", channel_id, message_id })`. 49: same pattern (a `Formatting` kind, a `DisplayHooks` method, a memo, the result on `MessageEntry`, drawing in its own timeline file, clicks through `PluginMenu` and `Link`). Real-Discord confirmation open: kept deletes and edits only exercised against mockcord.
