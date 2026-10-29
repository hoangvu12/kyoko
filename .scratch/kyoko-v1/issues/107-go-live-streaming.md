# 107 — Go Live streaming

**What to build:** Op 18 Create Stream, a stream connection identifying with one `screen` layer at 100, 105's capture encoded and sent, pause (op 22), and stop (op 19).

Plan ticket V11 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 105, 106

**Status:** claimed

- [ ] Session test (fake capture): Go Live sends op 18 with type `guild`, connects, announces op 12 and sends encrypted H264; viewers' wants are honoured; Stop sends op 19 and closes only the stream connection
- [ ] The LIVE badge shows on your own row and in the call view
