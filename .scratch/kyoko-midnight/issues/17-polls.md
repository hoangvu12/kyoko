# 17 — Poll creation and the Midnight poll

**What to build:** Create polls from the `+` menu with Discord's dialog (question, answers with emoji, add and remove, duration, multiple answers) and send them as the web client does. Restyle polls in the timeline: unvoted (radios, Vote, Show results), voted (bars, check, Remove Vote), ended (winner with a trophy, "Poll closed") and the poll-result system line with View Poll.

**Blocked by:** 16; v1 26

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/poll-create.html`, `docs/design/mockups/poll-in-messages.html`, `docs/design/mockups/poll-in-messages-ended.html`

**Spec:** `docs/design/midnight.md`, Composer extras

- [ ] Dialog and poll states match the mockups
- [ ] Creating a poll is a session command with a behaviour test against mockcord
- [ ] Limits (answer count, lengths, durations) match the web client
