# 10 — Paced read receipts and safe multi-delete

**What to build:** Reading channels sends read receipts at a human pace. Deleting several of your own messages never uses endpoints user accounts can't call.

**Blocked by:** 09 (REST safety layer)

**Status:** ready-for-agent

- [ ] Read acks are debounced per channel, never sent once per incoming message
- [ ] Marking a channel, a server or everything as read works and is paced
- [ ] Multi-delete issues paced single deletes, never the bulk-delete endpoint
- [ ] Session tests assert the ack and delete traffic
