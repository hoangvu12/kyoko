# 78 — Joining a server voice channel opens a DM call instead

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Clicking a voice channel in a server opens "that DM voice" instead (the DM call view). Joining a guild voice channel must open the guild voice channel's view with its participants.

**Blocked by:** none

**Status:** claimed

- [ ] Root cause found (routing from the voice channel click to the call view, or a voice state lookup keyed by the wrong id); fixed
- [ ] A test: joining a guild voice channel sends op 4 with that guild's id and channel id, and the UI shows that channel's call view, never a DM call; leaving works
- [ ] DM calls still work
