# 55 — ShowHiddenChannels and ShowHiddenThings

**What to build:** Users can see channels they can't access, with a lock icon and their name, topic, last activity and permission overwrites. They can also see moderator-only information the client already has. Both are display only.

**Blocked by:** 45 (Plugin runtime with ClearURLs), 23 (Channel sidebar)

**Status:** resolved

- [x] Hidden channels show in the sidebar with a lock icon and an info view
- [x] No message fetch or subscription is ever made for a hidden channel
- [x] ShowHiddenThings displays only data already present in state
- [x] Session tests assert zero traffic to hidden channels

## Answer

Merged to main as `d418c4e..0ec9037`. Docs: `docs/plugins.md` ("Hidden channels", "ShowHiddenChannels", "ShowHiddenThings", permission and slot rows), `docs/ui.md`, `docs/parity.md`.

- Session guard: every command that would read, subscribe to, ack or type in a channel the user can't view is dropped before the network (`State::is_hidden`, the covered list is `Command::reads_channel`: select, history, pins, threads, forum posts, send, reactions, typing, acks, mark read/unread, member-list viewports, channel search). The op 37 subscription with no channel open now picks only a viewable channel (it could pick a hidden one before).
- Sidebar: hidden channels are left out by default, as in Discord (they used to be listed locked); a hidden category disappears when none of its channels are listed. With ShowHiddenChannels they show in their category, locked and dimmed. Clicking one replaces the timeline and composer with an info view (name, kind, age-restricted and slowmode tags, topic, last activity from `last_message_id`, creation date, roles and members allowed or denied View Channel); the member-list panel hides while it's open; no command is sent. `timeline_slot` now takes `[forum_shown, hidden_shown]`.
- ShowHiddenThings: in the member list, a red timer for an active timeout (until-time tooltip) and a yellow shield for pending membership screening or an AutoMod username quarantine, from the member already in state (`bridge::moderator_notes`); nothing fetched.
- Plugins (`show-hidden-channels`, `show-hidden-things`, bundled, ship disabled) gate kyoko-drawn views through new slots `ui:sidebar.hidden_channels` and `ui:members.moderator_notes`, checked with `PluginHost::shows(slot)`; neither asks for `read:guilds`. Left out: Vencord's hidden voice preview, lock-icon settings, discovery/invite/mod-view options.
- Mockcord: `staff-only` already had the overwrites; the fixture's timed-out member now stays timed out (anchor + 400 days) and has the AutoMod flag (its moderation menu offers Remove Timeout).
- Tests: `session/tests/session/hidden.rs` sends every reading command for `staff-only` and asserts no request path, query, body or gateway frame names it, before and after a restart (fails without the guard); `bridge/tests/bridge/hidden.rs` with the real plugins; `ui::scripts::hidden`; `headless-run` reports `hidden_quiet: true`. Checks re-run by the orchestrator in lane C: all green.
- Parity (port 8493, kyoko only, `KYOKO_CAPTURE_PLUGINS=show-hidden-channels,show-hidden-things`): new `hidden-channel` and `member-notes`, checked by eye; plugins off, channel-sidebar and member-list unchanged by this ticket.
- Known gaps: a timeout that ends while the list is open stays marked until the row redraws; opening the info view (like switching to Home or another server) closes the timeline without telling the session, so the previously open channel can still be acked; a future "deselect" command would fix all three.
