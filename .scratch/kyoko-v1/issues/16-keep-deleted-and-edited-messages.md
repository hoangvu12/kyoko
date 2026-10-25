# 16 — Keep deleted and edited messages

**What to build:** Deleted messages stay visible in place with a deleted flag, and earlier versions of edited messages are kept, within limits, ready for MessageLogger.

**Blocked by:** 12 (Bounded state caches), 15 (Store hardening)

**Status:** resolved

- [x] Deleted messages are kept in place with a deleted flag instead of being removed
- [x] Previous versions of edited messages are kept
- [x] Retention is capped per channel and by age, and wiped on sign-out
- [x] State exposes the kept versions for rendering
- [x] Session tests cover delete and edit sequences and the caps

## Answer

Merged to main as `05db8f2`. Docs: "Kept deleted and edited messages" in `docs/storage.md`.

- State: `MessageView::deleted: Option<DeletedInfo { at }>`, `MessageView::versions: Arc<[MessageVersion]>` (oldest first; `MessageVersion { content, edited_timestamp, replaced_at, attachments, embeds }`). `State::deleted(channel, id)`, `State::edit_history(channel, id)`, `ChannelMessages::deleted_message/versions_of`. `MessageRow::Message` is now `Box<MessageView>`.
- Show/hide: `Command::ShowDeletedMessages(bool)` / `State::shows_deleted_messages()`, off at every start. Off: `channel_rows` omit deleted messages and `reply_target` gives `Unavailable`; on: deleted rows included and `ReplyTarget::Deleted(&Message)`. Versions are always on the view. A deleted message never counts as unread or places the unread marker.
- A version is kept when content changed, or attachments/embeds changed together with a new `edited_timestamp` (link unfurls aren't versions).
- Own deletes: a delete kyoko sent (`DeleteMessage(s)`, still `Queued`/`Deleted` in `State::deletions()`) is not kept (`State::deleted_by_me(id)`); own messages deleted elsewhere are kept.
- Store (migration 2, sealed): `kept_deleted (id, channel_id, deleted_at, json)` (moved out of `messages`), `kept_versions (id, message_id, channel_id, kept_at, json)`; wiped by 15's sign-out. `Retention`: `kept_deleted_per_channel` 100, `kept_versions_per_message` 10, `kept_versions_per_channel` 500, `kept_max_age` 7 days. `CacheLimits`: `kept_deleted` 100, `kept_edited` 100, `kept_versions` 10. Edits/deletes in cold channels go straight to the store.
- Fixed: a `last_message_id` pointing at a kept deleted message now counts as up to date (it made reloaded windows look detached).
- Mockcord `Control::edit_message`, `delete_message`, `bulk_delete`. 8 tests in `tests/session/kept.rs`.
- For 26: deleted style from `view.deleted`; handle `ReplyTarget::Deleted`; hidden deleted rows still occupy the channel window. For 45: expose `deleted` and `versions` to the render hook, plus `ShowDeletedMessages`. For 52: send `ShowDeletedMessages(true)` when enabled and on every start.
