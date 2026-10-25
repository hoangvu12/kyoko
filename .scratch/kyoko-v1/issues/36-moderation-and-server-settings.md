# 36 — Moderation and server settings

**What to build:** The moderation dialogs and server settings pages match oxidcord: overview, roles, members, channels, emoji, stickers, invites, bans and audit log.

**Blocked by:** 23 (Channel sidebar)

**Status:** resolved

- [x] Moderation and server settings screens match oxidcord in the parity captures
- [x] Edits are permission-checked and paced
- [x] The audit log and bans list load

## Comments

From the timeline (25): the Rust timeline view is drawn above the whole Luau tree, so any Luau menu, popover, modal or lightbox that overlaps the message area renders underneath it. Either draw that overlay in Rust above the timeline slot, or hide the slot while the overlay is open (see `docs/ui.md`).

## Answer

Merged to main as `d2367f2..0d82111`. Docs: `docs/ui.md` ("Server settings and moderation", "Root overlay layers"), `docs/rest.md`, `docs/parity.md`.

- Session: every server write (server, channels, roles, overwrites, emoji, stickers, invites, kick, ban, unban, timeout, nickname, role grants, voice moves) goes through one queue per server (`task/guild_writes.rs`): one write at a time, `RestTuning::guild_write_gap` 500 ms apart, a waiting edit merged into a newer one for the same target. Refused before any request when the permission or the role hierarchy says no (never the owner, only roles below your highest). Role, member and channel edits apply at once and roll back with a rejection on failure. Kick, ban and timeout send `X-Audit-Log-Reason` (`TimeoutMember` gained `reason`; the timeout dialog has no reason field, as oxidcord). Audit log `limit=50` then `before=<last id>`; bans `limit=1000` then `after=<last id>` (`LoadMoreBans`, `Loaded::Bans` gained `append` and `limit`). Seam tests in `tests/session/moderation.rs`.
- Window: server settings as a full-window view (`shell/server_settings.luau`, pages in `BUILDERS`, `bridge::ServerPage` with `ServerPage::allowed`, `bridge::GuildAccess`): overview, roles (with role editor), members, channels, emoji, stickers, invites, bans, audit log. 37's nav item and close button extracted to `ui/settings/frame.luau` (shared with `shell/settings.luau`). Moderation dialogs (nickname, timeout, kick, ban with focused reason) and the member menu (`bridge::moderation_items`, `moderation/menu.rs`) in the Rust `ModerationLayer`, registered in `layered(...)` and `src/ui/root.rs`. Sidebar `server_settings`, `create_channel`, `edit_channel`, `delete_channel` registered in `ui/server_settings.rs` (the `LATER` stub list is gone). `profiles.rs` `message_user`, `newest_other_author` are `pub(super)`. Edit Channel has overview fields only (no permission-overwrites tab); emoji can't be renamed inline; icon, emoji and sticker uploads are converted to PNG (animation lost). Each dialog sends exactly one command.
- Mockcord: 3 bans with audit entries and 2 invites in the first guild (oxidcord re-captured on the same fixtures).
- Verified in the running app via `Ui::dispatch` after 61: member right-click → Kick; Members page Roles submenu → role granted; sidebar Server Settings, Create Channel, Edit Channel, Delete Channel (removed #general). Headless test clicks Revoke Ban and a nav item. `headless-run` reports `moderation_ok: true`. Checks re-run by the orchestrator in lane B.
- Parity (port 8496, 150% scale): overview 1.6%, roles 0.9%, emoji 1.9%, stickers 0.8%, members 2.2%, invites 0.9%, bans 1.2%, audit-log 2.5%, moderation-nickname 6.9%, -timeout 6.8%, -kick 6.9%, -ban 7.2%.
- Small follow-ups (ticket 62): the Roles submenu opened from the member list at the right edge doesn't appear (probably opens past the window edge); roles the member already has show no check; deleting the open channel leaves the main column blank.
- For 45/plugins: member-menu entries in `bridge::moderation_items`, drawn in `moderation/menu.rs`; server pages in `BUILDERS` and `bridge::ServerPage`. Don't call a host action that reads state synchronously from a Luau effect: publishing holds the state read lock (the bans page deadlocked until the host deferred its republish to a task). For 55: server pages and member menu use `bridge::GuildAccess`/`ServerPage::allowed`; Edit Channel reads `state.channel`, so check Manage Channels per channel for hidden channels. For 33 follow-up: thread lock, delete, rename belong in the thread panel header gated by `ThreadHeader.can_manage`.
