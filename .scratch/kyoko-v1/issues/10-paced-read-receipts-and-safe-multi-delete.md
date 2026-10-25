# 10 — Paced read receipts and safe multi-delete

**What to build:** Reading channels sends read receipts at a human pace. Deleting several of your own messages never uses endpoints user accounts can't call.

**Blocked by:** 09 (REST safety layer)

**Status:** resolved

- [x] Read acks are debounced per channel, never sent once per incoming message
- [x] Marking a channel, a server or everything as read works and is paced
- [x] Multi-delete issues paced single deletes, never the bulk-delete endpoint
- [x] Session tests assert the ack and delete traffic

## Answer

Merged to main as `c73a68d`. Docs: "Read acks" and "Deleting messages" in `docs/rest.md`.

- Acks: one per-channel debouncer (`crates/session/src/task/reads.rs`) for opening a channel, new messages in the open channel, `AckChannel`, `MarkChannelRead`. Local read state updates immediately; only the REST ack waits. Viewing = selected channel, not `Command::SetViewing(false)` (default viewing), and last `SetViewport` at the bottom if any. `MarkUnread` sends `{"manual":true,"mention_count":0}` and suppresses auto-acks until reopened. Pending acks are dropped at shutdown.
- `MarkGuildRead` and new `MarkAllRead` use `POST /read-states/ack-bulk` (up to 100 channels per request), skipping channels without View Channel and only those behind or with mentions. Single ack: `POST /channels/{c}/messages/{m}/ack` with `{"token":null,"last_viewed":..}`.
- Deletes: `DeleteMessage` and new `DeleteMessages` (replaces `DeleteMessagesBulk`) share a sequential queue (`task/deletes.rs`). Own messages, or Manage Messages in a guild channel; others `Refused` (always in DMs). `State::deletions()` keeps the last 16 `DeleteBatch`es with `DeleteStatus { Queued, Deleted, Refused, Failed(SendFailure), Cancelled }`. 404 counts as deleted; any other failure cancels the rest. `RestClient::bulk_delete_messages` and the bulk-delete path are deleted.
- `RestTuning` additions: `ack_delay` 1.5 s + `ack_jitter` 0–1 s, `ack_max_delay` 10 s, `delete_gap` 1.2 s, `bulk_ack_gap` 2 s. New pacing lanes for single deletes and ack-bulk; `mutation_gap` still applies.
- Mockcord `Control::post_message(channel, author, content)`. 9 tests in `tests/session/reads.rs` and `deletes.rs`, each verified to fail with its mechanism removed.
- For 13: mockcord's `MESSAGE_ACK` echo overwrites read state without checking `version`; apply `MESSAGE_ACK` only when its version is newer. For 25: send `SetViewing(false/true)` on focus/minimise; don't send `AckChannel` per message. For 28: multi-select delete is `DeleteMessages { channel_id, ids }`, progress from `State::deletions().next_back()`. For 54: call `Command::MarkAllRead`.
