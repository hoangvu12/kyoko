# 108 — DM and group calls

**What to build:** Ring and accept, `CALL_CREATE`, and the DM call view, reusing 98-107. Not part of your first request.

Plan ticket V12 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** none

**Status:** resolved

## Scope (orchestrator, 2026-10-29; user said yes to planning and building it)

Discord's DM and group DM calls, reusing the voice stack from 97-107 and 116:

- [x] Start a call from a DM or group DM header (call button, and video call button that also turns the camera on): op 4 with `guild_id: null` and the DM `channel_id`, then the same voice connection as guild calls. Ring the others the way the web client does (research the exact REST calls, e.g. `POST /channels/{id}/call/ring` and `stop-ringing`, from the web client's behaviour; record them in `docs/voice.md`).
- [x] `CALL_CREATE` / `CALL_UPDATE` / `CALL_DELETE` in state (who's ringing, who's in, region), leniently decoded; shown in the DM list (call icon) and the channel.
- [x] Incoming call: Discord's ringing popup (caller avatar, Accept, Accept with video, Decline) with a ringtone through `crates/audio`/cpal, respecting the user's status (no ring sound in DND) and stopping when answered elsewhere, declined or timed out. Decline = stop ringing for you only.
- [x] DM call view: Discord's call area at the top of the DM (tiles, speaking rings, camera and Go Live tiles from 103-107, mute/deafen/camera/screen/leave controls), resizable as on Discord; the chat stays below.
- [x] Missed-call and call-ended system messages render like Discord (message types 3 `CALL`).
- [x] Session tests against mockcord (fake DM call server: CALL_CREATE ring, accept, decline, leave, the other side leaving, group DM with three people); never ring or call anyone real. Kyoko-only captures of the incoming-call popup and the DM call view, checked by eye against Discord screenshots or the design.
- Never auto-accept, auto-join or re-ring in a loop (plan section 7).

## Answer

Merged `e322367` (lane A, worker `884153cb`; commits `55946c4` Join, ring and decline DM and group calls, `e322367` Show incoming calls and the DM call view), verified by the orchestrator: fmt, clippy, build, xtask test, headless-run, deny, luau0-src 0.22.0+luau740 green; test --workspace green apart from load flakes that pass alone (`timeline::video_tests::a_video_attachment_plays_inline...`, `display::dont_round_my_timestamps...`, `voice_settings::high_quality_noise_suppression...`, and new `voice_processing::echo_cancellation_removes_the_calls_echo...`, added to rules.md); tls-capture chrome-156 0/0; kyoko-only captures `dm-call-incoming`, `dm-call`, `voice-call` checked by eye (no Midnight mockup).

- Start a call: op 4 with `guild_id: null`, then one `POST /call/ring` with `recipients: null`, only after Discord confirms the call and only if nobody is being rung yet. Joining a running call never rings; nothing re-rings. Decline = `stop-ringing` with `null` (only you stop ringing). A ring never joins by itself. Unanswered rings hide after a local 60 s timeout, nothing sent. Facts and sources in `docs/voice.md` "DM and group calls".
- CALL_CREATE/UPDATE/DELETE in state, lenient; DM rows show a call icon; type 3 call messages read started / lasted / missed. Fixed: gateway message edits dropped the `call` field, so a call message never learned the call ended.
- Incoming popup (Accept, Accept with Video, Decline) uses zgx `ui.layer`, not a Rust root layer (draws above the message list; noted in docs/ui.md). Ringtone `audio::Cue::looping(audio::ringtone)`; silent in DND (popup still shows) and under `--mock`.
- DM call view at the top of the DM, chat below, resizable from its bottom edge; `call_view.luau` gained an `embedded` mode.
- A voice state update for you from another device no longer moves this session's call (server calls too).
- Mockcord: fake DM call server, controls `ring_me`, `call_join_as`, `answer_elsewhere`; flags `--ring`, `--answer-calls`. Tests: 5 session (incl. a three-person group DM), 2 bridge, 2 Luau UI.
- Not done: the " — Join the call" link on a running call's message, an outgoing "calling" sound, the `GET /call` ring-eligibility check (endcord found it failing).
- **AUTO_CALL_CONNECT (IDENTIFY capabilities bit 12, 509 -> 4605) left out**, waiting on the user; parked on branch `lane-a-with-auto-call` (`770262c`). Without it a call already ringing when kyoko connects shows only on its next CALL_UPDATE. Settle the value from the real web client (74's Chrome 157 bump).
- For 102 (real call): check whether Discord rings the others by itself, the real ring timeout, and the web client's `capabilities` value.