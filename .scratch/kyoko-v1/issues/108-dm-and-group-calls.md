# 108 — DM and group calls

**What to build:** Ring and accept, `CALL_CREATE`, and the DM call view, reusing 98-107. Not part of your first request.

Plan ticket V12 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** none

**Status:** claimed

## Scope (orchestrator, 2026-10-29; user said yes to planning and building it)

Discord's DM and group DM calls, reusing the voice stack from 97-107 and 116:

- [ ] Start a call from a DM or group DM header (call button, and video call button that also turns the camera on): op 4 with `guild_id: null` and the DM `channel_id`, then the same voice connection as guild calls. Ring the others the way the web client does (research the exact REST calls, e.g. `POST /channels/{id}/call/ring` and `stop-ringing`, from the web client's behaviour; record them in `docs/voice.md`).
- [ ] `CALL_CREATE` / `CALL_UPDATE` / `CALL_DELETE` in state (who's ringing, who's in, region), leniently decoded; shown in the DM list (call icon) and the channel.
- [ ] Incoming call: Discord's ringing popup (caller avatar, Accept, Accept with video, Decline) with a ringtone through `crates/audio`/cpal, respecting the user's status (no ring sound in DND) and stopping when answered elsewhere, declined or timed out. Decline = stop ringing for you only.
- [ ] DM call view: Discord's call area at the top of the DM (tiles, speaking rings, camera and Go Live tiles from 103-107, mute/deafen/camera/screen/leave controls), resizable as on Discord; the chat stays below.
- [ ] Missed-call and call-ended system messages render like Discord (message types 3 `CALL`).
- [ ] Session tests against mockcord (fake DM call server: CALL_CREATE ring, accept, decline, leave, the other side leaving, group DM with three people); never ring or call anyone real. Kyoko-only captures of the incoming-call popup and the DM call view, checked by eye against Discord screenshots or the design.
- Never auto-accept, auto-join or re-ring in a loop (plan section 7).
