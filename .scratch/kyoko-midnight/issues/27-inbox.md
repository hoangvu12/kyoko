# 27 — Inbox

**What to build:** The header's inbox button opens Discord's Inbox popout, drawn in Rust above the timeline slot, with its tabs: **Mentions** (recent messages that mention you, your roles or @everyone across servers, newest first, fetched as the web client does when the tab opens, with server and channel context, Jump, and remove from the list), **Unreads** (channels with unread messages from kyoko's read state, grouped by server, with Jump and mark read on the paced ack path), and **For You** (only the entries kyoko has data for, such as friend request events; anything without data is left out and listed). Today the button does nothing. User decision, 2026-10-27: build it to the mockup.

**Blocked by:** Midnight 22; v1 10

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/inbox.html`

**Spec:** `docs/design/midnight.md`, Other surfaces

- [ ] The popout matches the mockup and is mapped in `MOCKUPS`
- [ ] Mentions are fetched only when the tab opens (paced, lenient decoding), with a session-seam test against mockcord
- [ ] Jump opens the message; mark read uses the paced ack path
- [ ] Escape and an outside press close it; it never covers the window when closed (`ui::root`)
