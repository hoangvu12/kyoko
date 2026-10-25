# 34 — Friends

**What to build:** The friends view matches oxidcord: online, all, pending, blocked, active now, and adding friends.

**Blocked by:** 24 (DM sidebar)

**Status:** resolved

- [x] Friends screens match oxidcord in the parity captures
- [x] Adding, accepting, removing and blocking work

## Answer

Merged to main as `6d182c3..3837ceb`. Docs: `docs/ui.md`, `docs/parity.md`, `docs/rest.md`.

- Session: RELATIONSHIP_UPDATE handled (type and nickname; `nickname: null` clears it; unknown ids or bad fields change nothing). `Command::AddFriend` takes `from: FriendSource`; from the Add Friend tab the POST carries `X-Context-Properties` `{"location":"Add Friend"}` built with 35's `ContextProperties`, from a profile no header. Accept, remove/ignore/cancel/unblock and block go through the existing paced mutation path.
- Bridge: `bridge::FriendsView` builds each tab's sections and counts (`ONLINE — n`, `INCOMING — n`, …), the name/username filter, the pending count and Active Now.
- Window: `shell/friends.luau` fills `home_page.luau` for `home.page == "friends"`: header and tabs, search, rows with Message and More (Remove Friend, Block, Copy User ID), Accept/Ignore, Cancel, Unblock, empty states, the Add Friend form with its success or refusal line, Active Now. Host side `src/friends.rs`, `src/ui/friends.rs`. A row click opens the profile popout through `Link::open_profile`; Message reuses the profile's DM path, so `OpenDm` stays behind the DM lock. Message Requests, Nitro and Shop stay placeholders.
- No kyoko lock on friend requests or blocks (user decision: do as Discord does). Remove Friend and Block confirmation dialogs are ticket 63.
- Tests: 4 session-seam tests in `tests/session/friends.rs` (exact Add Friend body and header, none from a profile; exact accept/remove/block/unblock requests without context header and spaced by at least `mutation_gap`; a captcha stops after one attempt; events move people between lists), 2 bridge tests, a `TestApp` test that every row button reaches its action without opening the profile, and an in-process `Ui::dispatch` run (accept, send, Message). Checks re-run by the orchestrator in lane A; `xtask test` green on a quiet machine (the earlier plugin-test failures were the known load flake).
- Parity (port 8494, 150% scale): friends-online 3.7%, friends-all 4.0%, friends-pending 3.1%, friends-blocked 2.8%, friends-add 2.9%. Remaining differences: mock avatar art, and oxidcord keeps the last DM highlighted where kyoko highlights Friends (24's behaviour).
- `tools/parity/oxidcord-ref.patch`: oxidcord's friends capture waits `SETTLE` after the DM opens (its selection sync closed the friends page again). Rebuild the reference after taking this.
- Unconfirmed on real Discord (in `docs/rest.md`): whether the web client sends a location context on accept/remove/block, and `PUT /users/@me/relationships/{id}` for a profile's Add Friend.
- For 41: `FriendsPage::reset()` runs beside the DM sidebar reset in `ui/sign_in.rs`; also reset the Add Friend outcome baseline (per window). For 51: the friends page doesn't touch mentions. For 63: add the dialogs on the More menu's Remove Friend and Block. Line heights: oxidcord's text labels use gpui's default line height (1.618 × size); matching it took the friends screens from about 4.5% to 3%, other Luau screens could gain the same.
