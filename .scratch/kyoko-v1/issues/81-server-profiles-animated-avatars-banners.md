# 81 — Server profiles, animated avatars and banners

**What to build:** Found by the user in the first real-account run (2026-10-27, Chrome-handshake build `35923bc`). Messages and profiles show the global profile: the user's server avatar, nickname and server banner aren't used (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_1.png` kyoko vs `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_2.png` Discord, same message). Animated avatars and banners don't animate (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_3.png` kyoko vs `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/Image_4.png` Discord).

**Blocked by:** none

**Status:** claimed

- [ ] Message rows, member list, popouts and full profiles use the member's server avatar, nick and server banner/bio where set (member `avatar`, `banner`; `guild_member_profile` in the profile response), falling back to global
- [ ] Animated avatars (`a_` hashes) animate where Discord animates them (on hover in message rows, always in popouts and the user panel, per the media playback setting); animated banners in profiles
- [ ] Tests at the bridge seam for the URL choice (server vs global, animated vs static); a kyoko capture checked by eye
