# 99 — DAVE end-to-end encryption

**What to build:** Port `dave/*` and wire it into 97's transform for Opus. Mockcord gains a DAVE server side from `dave::testing` and remote members that encrypt.

Plan ticket V3 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 97

**Status:** open

- [ ] With `dave_protocol_version: 1`, kyoko sends its key package, applies the welcome or commit, and sends transition-ready; the frames it sends carry the 0xFAFA marker and decrypt for the scripted member
- [ ] A member joining and leaving moves the epoch; media continues across the transition; an invalid commit sends op 31 and recovers
- [ ] Interop: a `davey` member (dev-dependency) and a kyoko member in one group decrypt each other's Opus frames
- [ ] Downgrade to 0 (op 21) and upgrade (op 24) handled; Opus silence frames pass through unencrypted
