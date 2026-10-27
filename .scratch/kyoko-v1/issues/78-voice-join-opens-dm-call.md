# 78 — Joining a server voice channel opens a DM call instead

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Clicking a voice channel in a server opens "that DM voice" instead (the DM call view). Joining a guild voice channel must open the guild voice channel's view with its participants.

**Blocked by:** none

**Status:** ready-for-human

- [ ] Root cause found (routing from the voice channel click to the call view, or a voice state lookup keyed by the wrong id); fixed
- [ ] A test: joining a guild voice channel sends op 4 with that guild's id and channel id, and the UI shows that channel's call view, never a DM call; leaving works
- [ ] DM calls still work

## Comments

Worker `08c2d9fd` (2026-10-27): kyoko has no voice (removed in 02; 23 opens a voice channel's text chat; no op 4 sender, no call view). On mockcord a voice-channel click opens that channel's own text chat, not a DM; the user most likely expected Discord's join. Options put to the user: (1) voice-channel view with participants and an explanatory Join button, no joining; (2) signalling-only join (shows the user in voice while silent; advised against); (3) real voice (voice websocket, UDP, Opus, DAVE: a ticket series). Waiting on the user.
