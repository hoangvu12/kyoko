# 130 — Animated avatars and banners animate like Discord

**What to build:** From the user's real run 4 (2026-10-30), screenshots in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy). Animated avatars still don't animate on real Discord (81 and 87 built animation on mock; find why real `a_` hashes don't). Discord's rules (image-18): animated avatars animate while hovered (member list, DM list, messages, voice members), and in an open profile popout or full profile; animated profile banners animate as soon as the profile opens. Image-17: a coloured background shows behind an avatar where the image is transparent; it must be transparent. Check the request URLs (`.gif`/`.webp?animated=true`, size) against the web client and against 93's fetch queue.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Root cause of the still avatars on real data stated, reproduced on mockcord with a real-shaped `a_` hash
- [ ] Hover animates avatars in every list; profiles animate avatar and banner on open; stills otherwise
- [ ] Transparent avatars have no background colour
- [ ] Session/bridge tests for the URL choice; in-process UI test for hover; captures checked by eye
