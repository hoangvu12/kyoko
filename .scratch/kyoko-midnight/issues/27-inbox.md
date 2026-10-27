# 27 — Inbox

**What to build:** The header's inbox button opens Discord's Inbox popout, drawn in Rust above the timeline slot, with its tabs: **Mentions** (recent messages that mention you, your roles or @everyone across servers, newest first, fetched as the web client does when the tab opens, with server and channel context, Jump, and remove from the list), **Unreads** (channels with unread messages from kyoko's read state, grouped by server, with Jump and mark read on the paced ack path), and **For You** (only the entries kyoko has data for, such as friend request events; anything without data is left out and listed). Today the button does nothing. User decision, 2026-10-27: build it to the mockup.

**Blocked by:** Midnight 22; v1 10

**Status:** resolved

**Mockups:** `docs/design/mockups/inbox.html`

**Spec:** `docs/design/midnight.md`, Other surfaces

- [x] The popout matches the mockup and is mapped in `MOCKUPS`
- [x] Mentions are fetched only when the tab opens (paced, lenient decoding), with a session-seam test against mockcord
- [x] Jump opens the message; mark read uses the paced ack path
- [x] Escape and an outside press close it; it never covers the window when closed (`ui::root`)

## Answer

Merged to main as `1ff4925..220680e`. Docs: `docs/rest.md`, `docs/ui.md` (inbox, what For You leaves out), `docs/parity.md`.

- Protocol (userdoccers, docs.discord.food Resources → User, "List Recent Mentions" and "Delete Recent Mention"; facts only, no identity): `GET /users/@me/mentions` with `before`, `limit` (1–100, default 25), `guild_id`, `roles` (default true), `everyone` (default true), messages from the past 7 days; kyoko sends `limit=25&roles=true&everyone=true` plus `guild_id` for "This Server Only". `DELETE /users/@me/mentions/{message.id}` returns 204 and fires `RECENT_MENTION_DELETE` (`{message_id}`). Mockcord returns server messages only, never DMs (believed to match Discord's Mentions tab; the docs don't say it outright).
- Session/state: fetched when the inbox opens on Mentions, when that tab is selected, or when the filter changes; at most once per 10 s for the same filter; `LenientVec` decoding (bad elements dropped). Removals are paced DELETEs; a gateway `RECENT_MENTION_DELETE` from another client removes one. `Changes` gained `recent_mentions`. `State::unread_channels()` and `bridge::unread_groups()` list viewable, unmuted, behind channels in rail order; `bridge::snippet_spans` marks mentions for pills.
- UI (`apps/desktop/src/inbox/`, root layer "inbox popout"): 480×600, `bg_3`, `radius_lg` (16, as the mockup), border, no shadow, at `header_popout_top` (70) with its right edge at the inbox button's. Tabs For You, Unreads, Mentions; channel group headers with server badge and X (removes every mention in the group, one paced DELETE each; the mockup has no per-card X); message cards with mention pills and Jump on hover. A filter menu (Include @everyone and @here, Include Role Mentions, This Server Only). Jump opens the channel like search results do (switching server, or opening the thread or DM) and closes the popout. Unreads grouped by server: Mark As Read sends `MarkChannelRead`, the per-server button `MarkGuildRead` (v1 10's paced path). For You shows only incoming friend requests with Accept and Ignore; everything kyoko has no data for is left out and listed in `docs/ui.md`. Escape and an outside press close it; Escape is in `escape_overlay`. Luau action `inbox.open {x, y}` (second press closes; home and friends page inbox buttons call it too). `threads::message_card` now takes the runtime and pictures (`CardHead`) so pins and the inbox share it. New tokens `inbox_popout_width`, `inbox_popout_height`; `funnel` added to the compiled icon list.
- Tests: session `mentions.rs` (nothing fetched before the tab opens; two opens give one GET with the web client's query; newest first with server and channel; paced removals with `assert_released_apart`; gateway delete; filter change refetches; malformed list keeps good elements); in-process `inbox::tests` (Jump closes the popout and mounts the message; Escape and an outside press close it); `top_bar_inbox` added to the `ui::root` press check.
- Checks re-run by the orchestrator in lane C, again after rebasing onto M15 (union conflicts in `ui.rs`, `ui/root.rs` `LAYERS` now 15, `docs/ui.md`, `docs/parity.md`, `MOCKUPS`): fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny all exit 0; luau0-src `0.22.0+luau740`. Mockup parity (port 8493, `pair.png` checked by eye): inbox 15.6% (mostly mockcord's channel content), apps-launcher 12.4% unchanged. The worker saw a new load flake, `display::dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one` (passed alone).
- Deviations: the header uses kyoko's outline inbox icon, not the mockup's filled one; role mentions in mockcord match only when the mock user's member holds the role.
- For v1 54 ReadAllNotificationsButton: send `Command::MarkGuildRead` or `MarkAllRead`, the same paced bulk-ack path Unreads uses; `unread_groups()` gives the data.
