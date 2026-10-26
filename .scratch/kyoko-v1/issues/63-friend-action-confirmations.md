# 63 — Friend action confirmations

**What to build:** Friend actions behave as in the official Discord client.

**Blocked by:** 34 (Friends)

**Status:** claimed

Decided with the user (2026-10-26): "do the same as Discord". Discord sends a friend request at once (it only steps in with a captcha, which 09 already handles by stopping and pointing to the official client), so kyoko adds no risky-action lock for friend requests. Discord asks for confirmation before removing a friend and before blocking someone, so kyoko does too.

- [ ] Remove Friend opens a confirmation dialog with Discord's wording before the request is sent; Cancel sends nothing
- [ ] Block opens a confirmation dialog with Discord's wording before the request is sent; Cancel sends nothing
- [ ] Sending, accepting, ignoring and cancelling friend requests stay one click with no kyoko lock, as in Discord
- [ ] The dialogs are Rust dialogs (`motion::modal`, a root layer added the 61 way, or 35's `CommunityLayer`), covered by a headless or in-process test that Cancel sends nothing and Confirm sends exactly one request
