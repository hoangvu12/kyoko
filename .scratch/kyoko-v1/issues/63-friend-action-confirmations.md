# 63 — Friend action confirmations

**What to build:** Friend actions behave as in the official Discord client.

**Blocked by:** 34 (Friends)

**Status:** resolved

Decided with the user (2026-10-26): "do the same as Discord". Discord sends a friend request at once (it only steps in with a captcha, which 09 already handles by stopping and pointing to the official client), so kyoko adds no risky-action lock for friend requests. Discord asks for confirmation before removing a friend and before blocking someone, so kyoko does too.

- [x] Remove Friend opens a confirmation dialog with Discord's wording before the request is sent; Cancel sends nothing
- [x] Block opens a confirmation dialog with Discord's wording before the request is sent; Cancel sends nothing
- [x] Sending, accepting, ignoring and cancelling friend requests stay one click with no kyoko lock, as in Discord
- [x] The dialogs are Rust dialogs (`motion::modal`, a root layer added the 61 way, or 35's `CommunityLayer`), covered by a headless or in-process test that Cancel sends nothing and Confirm sends exactly one request

## Answer

Merged to main as `f0af011`. Docs: `docs/ui.md` (actions and the confirmations paragraph), `docs/parity.md`.

- New Rust root layer `apps/desktop/src/friend_dialog.rs` (`FriendDialog`, `FriendConfirm`), added the 61 way (`layered(...)` in `ui.rs`, `LAYERS` and the `layer` match in `ui/root.rs`), built with `kit::dialog(motion::modal(...))` like `delete-message-dialog`. Confirm sends `RemoveRelationship` or `BlockUser`; Cancel, the X and Escape send nothing.
- One confirmation path: `Link::ask_remove_friend` and `Link::ask_block` in `ui/friends.rs`. The friends page More menu calls the new host actions `friends.ask_remove` / `friends.ask_block` (`friends.block` is gone); the profile's Block closes the profile and sends `profile::Request::Block(id)`, which opens the same dialog (M20's More menu was rebased onto this by the orchestrator). Accept, Ignore, Cancel, Unblock and Add Friend stay one click.
- Wording: Remove: "Remove '<name>'", "Are you sure you want to permanently remove <name> from your friends?", red "Remove Friend". Block: "Block '<name>'?", "Are you sure you want to block <name>? Blocking will also remove them from your friends list." (second sentence only for a friend), red "Block". Only partly confirmed from public sources: "permanently" and the Block explanation need a look at the live web client some day. Names are plain, not bold.
- Test: `friend_dialog::tests::friend_confirmations_send_nothing_on_cancel_and_one_request_on_confirm` (TestApp plus a real session against a recording mockcord: Cancel makes zero relationship requests, Confirm exactly one PUT or DELETE, held 600 ms). `the_friends_page_reaches_every_relationship_action` asserts request buttons never confirm.
- Kyoko-only captures `friend-remove-dialog`, `friend-block-dialog`. Verified by the orchestrator in lane B after rebasing onto M16 (union conflicts in the layer lists): all checks green (`storage::turning_the_opt_in_on_and_off_moves_the_secrets` flaked once under load, the rerun passed).
- For later tickets: any new menu that removes or blocks someone (member menu, DM menu) calls `Link::ask_remove_friend` / `Link::ask_block` or sends `profile::Request::Block`, never the commands directly. Another person-confirmation adds a `FriendConfirm` variant and calls `link.friend_dialog.ask(...)`.
