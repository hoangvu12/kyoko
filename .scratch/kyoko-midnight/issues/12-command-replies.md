# 12 — Command replies: thinking, ephemeral and failures

**What to build:** Show interaction replies as Discord does: the interaction header ("hoangvu12 used /roll"), the pending "Sending command..." row, "<App> is thinking..." for deferred replies, ephemeral messages with the mention gradient and "Only you can see this · Dismiss message", "The application did not respond" when no reply comes in time, and "This interaction failed" under components. Ephemeral messages live only in memory and go away when dismissed.

**Blocked by:** 05, 10

**Status:** resolved

**Mockups:** `docs/design/mockups/command-response-states.html`

**Spec:** `docs/design/midnight.md`, Command replies and components

- [x] Each state matches the mockup
- [x] The timeout follows the web client
- [x] Dismiss removes an ephemeral message locally
- [x] Session-seam tests cover deferred, ephemeral and failed replies with mockbot

## Answer

Merged to main as `3a4f175`. Docs: `docs/rest.md` (interaction behaviour), `docs/ui.md` (timeline rows), `docs/testing.md` (Mockbot), `docs/parity.md`.

- Rows: running a command adds a pending row at once with the "<user> used /roll" header (16-point avatar, reply spine, command in `link`), the app as author with the tag (now **APP** instead of BOT, as in every mockup), "Sending command..." at 50%. `INTERACTION_CREATE` turns it into "<App> is thinking..." with dots; a deferred reply arrives flagged `LOADING` and keeps thinking until the follow-up edit; the real reply carries the nonce and replaces the pending row.
- Ephemeral replies (flag 64): mention wash and bar, eye icon, "Only you can see this ·" and "Dismiss message"; no hover bar or menu. Kept only in memory (never in the store), per channel (newest 50) as the web client's `EphemeralMessageStore`, survive a channel switch, gone on Dismiss (`Command::DismissMessage`), `MESSAGE_DELETE`, a new gateway session (READY, not RESUMED), an account switch or restart.
- Failures show in the row as ephemeral red text with Dismiss (no toast): "The application did not respond" for every timeout, or the refusal reason when the request fails; a failed pending row is dismissed with `DiscardMessage`. Timeouts from Discord's web bundle (fetched with UA `kyoko-dev`, no cookie or identity): `POST /interactions` without an answer in 3 s fails; `INTERACTION_FAILURE` fails; a `LOADING` reply older than 15 minutes (by snowflake) times out (the session wakes at the deadline). The 15-minute path has no session test (would need 15 minutes).
- Prepared for M13: `LocalEvent::ComponentInteraction { channel_id, id, failed }` and `MessageEntry::interaction_failed`, which draws "This interaction failed" at the end of the row; nothing sets it yet.
- Mockcord: Mockbot `/think` (follow-up after 2 s), `/whisper` (ephemeral), `/ignore` (failure after 3 s); `/roll` answers "🎲 You rolled a N" (sides capped at 17); `InteractionTiming`, `Control::set_interaction_timing`, flags `--slow-followups`, `--hold-ping`; replies carry the nonce and `interaction_metadata`.
- Tests: session `command_replies.rs` (four: sending then replaced; deferred; ephemeral in memory, across a channel switch, not in the store, Dismiss, gone after a new session and a restart; failures `/broken`, `/ignore`, a captcha refusal and a held request timing out after 3 s with no toast). The old `a_command_the_application_fails_is_reported_once` is gone (failures are no longer toasts).
- Checks re-run by the orchestrator in lane A on `cc2117e`: all green. Mockup parity (port 8494): command-response-states 14.9%; command-options 13.1%, command-choices-and-autocomplete 12.0%, timeline 17.5% (unchanged).
- Deviations: pending rows still sort after confirmed messages (the web client orders local messages by creation time; changing it affects normal sends), so "Sending command..." sits fourth instead of first; the states use `/think`, `/whisper`, `/ignore`, `/ping` instead of `/roll`; the per-reason failure strings from the web client's locale file aren't used.
- For M13–M15: M13 sets `LocalEvent::ComponentInteraction` when a component interaction fails; components reuse `Interactions::remember`/`settle` and `Internal::CommandFailed`; the mockup's interaction-failed row isn't staged in the capture yet. After `INTERACTION_SUCCESS` with no message the pending row stays "thinking"; the web client removes it on `INTERACTION_MODAL_CREATE`, so whoever builds modals does that. M14: built-in commands don't create pending rows.
