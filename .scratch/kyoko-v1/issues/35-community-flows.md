# 35 — Community flows

**What to build:** Adding or joining a server, invites, forwarding messages and onboarding match oxidcord. Onboarding has no artificial delay, which is the built-in version of NoOnboardingDelay.

**Blocked by:** 23 (Channel sidebar)

**Status:** resolved

- [x] Community screens match oxidcord in the parity captures
- [x] Joining by invite, creating invites and forwarding messages work
- [x] Onboarding completes without artificial waits and respects the risky-action locks

## Comments

From the spike (04): joins and invite acceptance need the web client's `X-Context-Properties` header; add it to the fingerprint module when this ticket builds those requests.

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `1cdc281..88173fb`. Docs: `docs/ui.md` ("Community dialogs"), `docs/rest.md`, `docs/fingerprint.md`, `docs/locks.md`, `docs/parity.md`.

- Session (`task/community.rs`): invite lookup `GET /invites/{code}?with_counts=true&with_expiration=true` (404 marks the code unknown); join `POST /invites/{code}` with `{"session_id"}` (`{}` before READY) and `X-Context-Properties` (`protocol::fingerprint::ContextProperties`, `JoinLocation`; sent only on joins); after a join the rules and onboarding load at once. Rules submit `PUT /guilds/{id}/requests/@me` (version plus form fields with `response: true`); onboarding `PUT /guilds/{id}/onboarding-responses` with the user's picks and seen timestamps (unknown option ids dropped; a required prompt left empty is refused before any request). Invite creation `POST /channels/{id}/invites` with exactly `{max_age, max_uses, temporary}`; server creation `POST /guilds` with the web client's template body (`guild_template_code` `2TffvPucqHkN`). Forward: per target a lock check, Send Messages check, fresh nonce and a type-1 `message_reference`, one send gap apart. A captcha on join stops with a rejection and is sent once. Screening and onboarding lock texts now say to open the server.
- State and bridge: last join, loaded onboarding and rules, unknown invite codes; pure helpers for invite codes, forward targets and ranking, notification levels, invite channel and server gates.
- Window: Rust `CommunityLayer` (`apps/desktop/src/community/`) in the root `overlay()` inside a zero-size absolute box: Add a Server (join with preview card, or create with icon upload), Forward To (click picks up to 5 targets, Send appears; Enter with nothing picked forwards to the first match; preview shows mention names), Notification Settings, Invite People, Leave Server, server rules, onboarding. Server and channel menus wire Invite, Notification Settings, Leave, mute submenus and a channel notification-level submenu. After a join, or when opening a server whose rules or onboarding aren't done, the server opens and its rules then onboarding show immediately. Only plain rules forms are submitted; other question types point to the official client. No invite cards in messages yet.
- Tests: 5 seam tests in `tests/session/community.rs` (join header, session id and lookup query, no header elsewhere; captcha stop; invite body; forward pacing, nonces, reference, locked target held; onboarding within 500 ms of the join, locked until the user's submit, required prompt refused, exact Finish body, unlocked after). Two headless Luau menu-wiring tests in `scripts.rs` (Luau clicks were broken in-process, ticket 61). Join → rules → onboarding → unlocked and a two-target forward driven through the Rust dialogs in the running app. `headless-run` reports `community_ok: true`. Mockcord invite `mockcommunity` with rules, onboarding and leave; content-less forwards accepted. Checks re-run by the orchestrator in lane A.
- Parity (port 8494, 150% scale): add-server 7.2%, forward 7.3%, guild-notifications 6.3%; rules and onboarding have no oxidcord screens (drawn in its dialog style).
- To confirm on real traffic (not run; joins are actions): the `X-Context-Properties` shape (undocumented), `inputValue` on the invite lookup, `flags` on invite creation, the join body outside guest invites.
- For 36: `sidebar.rs` `LATER` holds only `server_settings`, `create_channel`, `edit_channel`, `delete_channel`. To add a dialog to `CommunityLayer`: a `Dialog` variant and module, reuse `parts.rs` (`panel`, `field`, `segmented`, `switch_row`, `footer`, `labelled_button`), open with `community.open(...)`; text-field dialogs use the autofocusing modal (`typing`). Menu items need ids for headless tests; submenus need `trigger = ui.menu_item { label, id }`. For 41: reset the per-window join and gate tracking in `Link.joins` on account switch. For an invite-card follow-up: send `AcceptInvite { from: InviteSource::Message { channel_id, message_id } }`.
