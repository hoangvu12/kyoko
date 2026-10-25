# 11 — Risky-action locks

**What to build:** Before actions that commonly trip Discord's spam defences, kyoko locks the action, explains why and asks for confirmation. The safety setting can turn this off.

**Blocked by:** 09 (REST safety layer)

**Status:** resolved

- [x] Locks cover new DMs to strangers, message-request and spam DMs, empty channels, and server screening, onboarding and verification gates
- [x] The locked state and its reason are exposed in state for the UI
- [x] A single safety setting, on by default, turns the locks off after a restart
- [x] Session tests cover each lock and the setting

## Answer

Merged to main as `3b3a764` (rebased over 10 by the orchestrator). Docs: `docs/locks.md`.

- Locks are checked in the session (`task/locks.rs`) before any REST request: a locked action is dropped, the UI gets `CommandRejected` with the explanation, no pending message is created.
- `state::RiskyReason` (`code()`, `explanation()`, `is_gate()`): `stranger_dm` (not a friend, no history: relationship type 1, `recipient_ids`, `last_message_id`/history), `message_request` (`is_message_request`), `spam_dm` (`is_spam`), `empty_channel` (null `last_message_id` or empty history), `screening` (member `pending`), `onboarding` (guild `GUILD_ONBOARDING` + member flag bit 3 set and bit 1 unset), `verification` (level 1 unverified email, 2 account < 5 min, 3 joined < 10 min, 4 READY `phone: null`; never for owner, members with roles, or flag bit 2). Missing fields never lock. Precedence: gates before `empty_channel`; `spam_dm` before `message_request` before `stranger_dm`.
- State: `State::channel_lock(channel_id, now)`, `State::dm_lock(user_id)`, `RiskyLock { target: RiskyTarget::Channel | User, reason, until }`, `State::safety_locks()`, `State::is_risky_confirmed(..)`, `Changes::locks`. A channel lock holds send/sticker/GIF/forum post/thread/reaction/typing (typing dropped silently) and forwards into it; a user lock holds `OpenDm`.
- `Command::ConfirmRisky { target, reason }` lifts one reason on one target for the session (confirming `stranger_dm` for a user covers the DM channel). The UI re-sends the held command. Gates can be confirmed too; Discord still enforces them.
- Setting `Settings::safety_locks` (top level of the store's `settings.json`, default true), read once at `Session::start`.
- Protocol: `User.phone` (lenient, missing vs null distinct), member flag constants, `GuildState::verification_level`; a partial member no longer wipes `pending`/`flags`/`joined_at`. Not locked: creating a group DM with strangers.
- 8 tests in `tests/session/locks.rs`.
- For 27: read `channel_lock` before sending; show `explanation()` and a confirm button sending `ConfirmRisky`, then re-send; re-read on `Changes::locks` and at `until`; for `is_gate()` point to the official client. For 24: check `dm_lock(user_id)` before `OpenDm`. For 35: kyoko never submits screening/onboarding. For 37: "Safety locks" toggle bound to `Settings::safety_locks`, at your own risk, applies after restart. For 21: a new sign-in re-reads the setting and clears confirmations.
