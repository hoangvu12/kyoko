# 104 — Camera send

**What to build:** Port the WGC/MF camera capture and MF H.264 encode with two simulcast layers (100, 50). Op 12 announce and clear, `self_video` in op 4, honouring server wants, and a camera preview in settings.

Plan ticket V8 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 103

**Status:** claimed

- [ ] Session test (fake capture source): Camera on sends op 4 `self_video: true`, op 12 with the Ready-assigned SSRCs for both layers, and DAVE-encrypted H264 with playout-delay; wants of 0 pause a layer; Camera off clears
- [ ] MF unavailable (N edition) shows "video unavailable" without breaking audio
