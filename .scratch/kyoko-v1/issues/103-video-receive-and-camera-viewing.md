# 103 — Video receive surface and camera viewing

**What to build:**
- Port the video RTP depacketizer, MF H.264 decode and the receive path.
- Op 12 announcements and per-SSRC Media Sink Wants with `pixelCounts` from tile sizes.
- An `apps/desktop` video surface feeding `image_signal`.
- Spike first: frame upload cost in zgx.

Plan ticket V7 of `nguyenvu/.research/voice-plan/plan.md` (read its sections 1–5 and 7 for the facts and the port list). Supersedes 78.

**Blocked by:** 101

**Status:** open

- [ ] Session test: a scripted member's op 12 plus H264 fixture frames (DAVE on) produce decoded frames for that user; a PLI is sent on loss; op 13 ends the tile
- [ ] Wants: a focused tile asks for 100, a thumbnail 50, off-screen 0
- [ ] Measured CPU and GPU upload for 1 and 4 tiles at 720p30; the frame rate is capped if needed; written down in `docs/media.md`
