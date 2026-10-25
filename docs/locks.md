# Risky-action locks

Some ordinary actions are known to trip Discord's spam defences, especially on new or low-reputation accounts: messaging someone you have never talked to, answering message requests or spam, being the first to post in a channel, and talking in a server whose membership gates you haven't passed. Kyoko locks these actions, says why, and lets the user go ahead only after an explicit confirmation.

Locks live in the session, in front of the REST layer (`docs/rest.md`). A locked command is dropped before any request is built, so nothing reaches the wire. Kyoko never completes screening, onboarding or verification on the user's behalf, and never confirms a lock by itself.

## Reasons

Every lock has a `state::RiskyReason`. `code()` is a stable string for the UI and translations, and `explanation()` is a short English sentence the UI can show as is. `is_gate()` is true for the three server gates, whose explanation points to the official client.

| Code | Applies to | Locked when |
|---|---|---|
| `stranger_dm` | 1:1 DM channels, and opening a DM with a user | the other user is not a friend (relationship type 1) and the conversation has no history |
| `message_request` | DM and group DM channels | the channel's `is_message_request` is `true` (READY `private_channels`, CHANNEL_CREATE, CHANNEL_UPDATE) |
| `spam_dm` | DM and group DM channels | the channel's `is_spam` is `true` (same sources) |
| `empty_channel` | guild text and announcement channels | the channel has no history |
| `screening` | every channel of a guild | your member object has `pending: true` |
| `onboarding` | every channel of a guild | the guild has the `GUILD_ONBOARDING` feature and your member `flags` have STARTED_ONBOARDING (1 << 3) but not COMPLETED_ONBOARDING (1 << 1) |
| `verification` | every channel of a guild | the guild's verification level is not met (below) |

"No history" means: once history has loaded, no messages came back; before that, the channel's `last_message_id` is null. Any message in the cache, from any author, ends the lock.

The verification gate mirrors Discord's levels. It never applies to the guild owner, to a member with any role, or to a member with BYPASSES_VERIFICATION (1 << 2):

- Low (1): your account's `verified` is `false`.
- Medium (2): your account is less than 5 minutes old (from your user id).
- High (3): you joined the guild less than 10 minutes ago (member `joined_at`).
- Very high (4): your READY user has `"phone": null`.

The two timed cases set `RiskyLock::until` to the moment the gate lifts.

Missing or malformed fields never lock: an absent `is_spam`, `pending`, `flags`, `phone`, `verified` or member object counts as "not gated". A guild gate takes precedence over `empty_channel`; `spam_dm` over `message_request` over `stranger_dm`.

## Locked actions

A channel lock holds `SendMessage`, `SendSticker`, `SendGif`, `CreateForumPost`, `CreateThread`, `AddReaction` and `Typing` in that channel, and `ForwardMessage` into it. A user lock (`stranger_dm`) holds `OpenDm`, which would otherwise create the DM channel. A held command publishes a `CommandRejected` with the lock's explanation (`State::last_rejection`), except `Typing`, which is dropped silently. Pending messages are not created for held sends.

## State

- `State::channel_lock(channel_id, now) -> Option<RiskyLock>`: the lock on the composer and message actions of a channel. Pass `Utc::now()`.
- `State::dm_lock(user_id) -> Option<RiskyLock>`: the lock on starting a conversation with a user, for "Message" buttons on profiles and the member list.
- `RiskyLock { target, reason, until }`, with `target` a `RiskyTarget::Channel(id)` or `RiskyTarget::User(id)`.
- `State::safety_locks()`: whether locks are on for this session.
- `State::is_risky_confirmed(target, reason)`.

Lock inputs change with ordinary events, so re-read the open channel's lock whenever its messages, the private channels, the guild's channel list or members, the relationships or `Changes::locks` change. For a lock with `until`, re-read it at that time.

## Confirming

The UI shows the explanation and a confirm button. Pressing it sends:

```rust
Command::ConfirmRisky { target: lock.target, reason: lock.reason }
```

The confirmation lifts that one reason on that one target for the rest of the session, and sets `Changes::locks`. It does not carry over to other channels, other reasons or the next launch. Confirming `stranger_dm` for a user also lifts `stranger_dm` on the DM channel with that user, so "open DM" and its first message need one confirmation. The UI then re-sends the original action itself; the session does not replay held commands.

Confirmations are only ever a user gesture. Plugins must not send `ConfirmRisky`.

## Rules and onboarding

The screening and onboarding gates cannot be confirmed away by kyoko. Instead the interface shows Discord's own screens when the user joins or opens such a server (`docs/ui.md`, "Community dialogs"): the server's rules, and the onboarding prompts, with no artificial delay. Only the user's own Submit or Finish sends them (`AcceptMemberVerification`, `SubmitOnboarding`, `docs/rest.md`), with exactly what the user picked. Until Discord confirms the member (GUILD_MEMBER_UPDATE), the gate keeps holding sends, reactions and forwards in every channel of the server.

## The setting

`Settings::safety_locks` in the store's `settings.json`, `true` by default. When `false`, no lock applies. The session reads it once at start, so a change takes effect after a restart (or sign-in). The Privacy & Safety settings page shows it under "Safety Locks" as "Hold risky actions", with the warning that turning it off is at the user's own risk and applies after a restart.
