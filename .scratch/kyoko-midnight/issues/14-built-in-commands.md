# 14 — Built-in slash commands

**What to build:** Support all of Discord's built-in commands, listed under Built-In as captured on the live client: `/gif query` and `/sticker` (results as a strip above the composer; Enter or a click sends), `/me`, `/shrug`, `/tableflip`, `/unflip`, `/spoiler` (change the message text as Discord does), `/msg` (opens or creates the DM and sends), `/nick` (changes your nickname in the server), `/thread` (creates a thread with a first message), and `/ban`, `/kick`, `/timeout` when you have the permission (through the same paths and confirmations as kyoko's moderation dialogs). In the picker only `/gif` shows an option chip. `/schedule` ("Schedule a message to send later") is new in Discord: research how the web client schedules a message before building it; if it cannot be done the way the web client does, leave it out and say so in the answer.

**Blocked by:** 10; 08 for the GIF and sticker strips

**Status:** ready-for-agent

**Mockups:** `docs/design/mockups/command-picker-built-in.html`, `docs/design/mockups/built-in-gif.html`, `docs/design/mockups/built-in-sticker.html`, `docs/design/mockups/built-in-thread.html`

**Spec:** `docs/design/midnight.md`, Command picker and options

- [ ] Every built-in except possibly `/schedule` works against mockcord, with a session-seam test where it reaches the network
- [ ] Moderation built-ins appear only with the permission and confirm as the dialogs do
- [ ] `/schedule` is either built the web client's way or documented as left out
