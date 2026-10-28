# 101 — Call view, speaking rings and per-user controls

**What to build:** `bridge::voice` participants and speaking; the call view's audio tiles; speaking rings in the sidebar rows; the participant menu with user volume and local mute; the Opus bitrate follows the channel's `bitrate`.

Plan ticket V5 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 98

**Status:** claimed

- [ ] Session test: op 5 from a scripted member shows that user speaking and stops after silence; op 11 and 13 add and remove tiles
- [ ] User volume scales that user's mixed output (manual sink); local mute silences it
- [ ] Opening the voice channel you are in shows the call view; Open Chat shows its text chat
