# 17 — Poll creation and the Midnight poll

**What to build:** Create polls from the `+` menu with Discord's dialog (question, answers with emoji, add and remove, duration, multiple answers) and send them as the web client does. Restyle polls in the timeline: unvoted (radios, Vote, Show results), voted (bars, check, Remove Vote), ended (winner with a trophy, "Poll closed") and the poll-result system line with View Poll.

**Blocked by:** 16; v1 26

**Status:** resolved

**Mockups:** `docs/design/mockups/poll-create.html`, `docs/design/mockups/poll-in-messages.html`, `docs/design/mockups/poll-in-messages-ended.html`

**Spec:** `docs/design/midnight.md`, Composer extras

- [x] Dialog and poll states match the mockups
- [x] Creating a poll is a session command with a behaviour test against mockcord
- [x] Limits (answer count, lengths, durations) match the web client

## Answer

Merged to main as `93be856..1c4353e`. Docs: `docs/rest.md`, `docs/ui.md` (ids), `docs/parity.md`, `THIRD_PARTY_NOTICES.md` (the mockup's `poll.svg`).

- Create: Create Poll in the `+` menu opens Discord's 570-wide dialog (`poll_dialog.rs`, owned by `ComposerLayer`, which now takes `Pictures` and has its own emoji picker): question with "n / 300" counter, answers with emoji button and remove, "Add another answer" up to 10, duration select (24 hours default), Allow Multiple Answers, Post dimmed until valid. `Command::SendPoll` goes through the normal send path (locks, pacing, pending row, Retry) and checks Send Messages + Create Polls. Body: `nonce` plus `poll: {question:{text}, answers:[{poll_media:{text, emoji?}}], allow_multiselect, duration, layout_type: 1}`; Unicode emoji as `{name}`, custom as `{id}`; blank answers dropped, text trimmed.
- Limits: question 1–300 characters, 1–10 answers of at most 55, durations 1h, 4h, 8h, 24h, 3 days, 1 week, 2 weeks. Sources: discord-userdoccers `resources/message` (300, 55, 10, durations in hours up to 32 days, vote and expire routes), Discord's Polls FAQ (1 hour to 1 week); 2 weeks from the mockup. Bad drafts are refused before sending; the dialog's fields stop at the same lengths. `session` exports `PollDraft`, `POLL_*` limits, `POLL_DURATIONS`.
- Votes: `PUT /channels/{c}/polls/{m}/answers/@me` with `{"answer_ids": [...]}`; removing a vote is the same `PUT` with an empty list (userdoccers), not a `DELETE`. Live counts from `MESSAGE_POLL_VOTE_ADD`/`REMOVE` and message updates; a malformed vote event is dropped. End Poll on your own open polls already existed (`EndPoll`).
- Timeline (`timeline/polls.rs`): unvoted (radios, checkboxes for multiple answers, Show results, disabled Vote), voted (bars, check, Remove Vote), ended (winner in green with a trophy, "Poll closed"); `PollResult` messages are system rows (`SystemIcon::Poll`) ending with View Poll, which jumps to the poll. Bridge: `PollSummary` (winner, `footer()`, time left), `PollOutcome` (from the result message's embed), `can_create_polls`.
- Mockcord: accepts `poll` on create, sends vote events, posts the poll result message when a poll ends (`tools/mockcord/src/polls.rs`); showcase sections `showcase: polls` and `showcase: ended poll` before reactions; open polls expire relative to the real clock rounded to the hour (the fixture date is fixed at 2026-10-01).
- Tests: six session tests in `polls.rs` (exact create body and pending row, votes and removal, live counts, limits refused before sending, paced sends respecting locks via `assert_released_apart`); in-process `ui::root::the_plus_menu_opens_the_poll_dialog_which_posts_a_draft_within_the_limits`.
- Checks re-run by the orchestrator in lane B after rebasing onto M13 (union conflicts in `session/command.rs`, `state/event.rs`, `capture.rs`, `timeline/icons.rs`, `ui.rs`, `docs/parity.md`, the mockcord showcase, `MOCKUPS`): fmt, clippy, build, `test --workspace`, `xtask test`, `headless-run --mock`, deny all exit 0; luau0-src `0.22.0+luau740`. Mockup parity (port 8496, `pair.png` checked by eye): poll-create 8.2%, poll-in-messages 13.7%, poll-in-messages-ended 13.4%; message-components 15.8% and app-modal-error 10.7% unchanged after the merge.
- Deviations: the label back from Show results is "Go back to vote" (no source found); the dialog dims the window like other kyoko dialogs (the mockup has no dim); two polls in a row from one author group under one header; the answer emoji picker opens at the dialog's right edge and covers part of it; timeline poll row heights are estimates. Open real-Discord confirmation: whether the web client also sends `content: ""`, `tts` or `flags` with a poll.
- For M15: the `+` menu has no Use Apps entry yet.
