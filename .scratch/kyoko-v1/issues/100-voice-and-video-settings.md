# 100 — Voice & Video settings

**What to build:** The Settings page ungreyed: devices, volumes, mic test, voice activity or push to talk with a hotkey, input sensitivity (auto or threshold), noise suppression. Settings persist in `store::VoiceSettings`.

Plan ticket V4 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 98

**Status:** open

- [ ] Session tests: a device change while connected restarts audio without reconnecting; the threshold gates sending; push to talk sends only while held; noise suppression toggles the chain
- [ ] Mic test runs without joining; the level meter updates; settings survive a restart
- [ ] Captures checked by eye; a page text added to `SETTINGS_PAGES` for the load check
