# 129 — Voice channel chat beside the call

**What to build:** From the user's real run 4 (2026-10-30), image-14 (Discord) vs image-15 (kyoko) in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy). In Discord, opening the chat of the voice channel you are in shows the call tiles on the left and the voice channel's text chat in a right-hand panel (with its own header and close button). Kyoko shows the text chat full width and no call. Match Discord: the call view (101) keeps its place, the chat opens as a side panel, closing it gives the call the full width; opening a voice channel's chat while not connected keeps today's full-width chat. Remember the overlay rule: the Rust timeline must fit the panel's slot.

**Blocked by:** 128 (same voice sidebar and call view files)

**Status:** ready-for-agent

- [ ] Connected + Open Chat shows call tiles left, voice chat panel right, like image-14; closing the panel restores the full call
- [ ] Not connected: Open Chat keeps the full-width chat
- [ ] Composer, replies and scroll work inside the panel
- [ ] In-process UI tests; kyoko-only captures checked by eye against image-14
