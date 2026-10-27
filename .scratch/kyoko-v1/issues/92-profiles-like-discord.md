# 92 — Profiles like Discord: DM panel, full profile, popout menu

**What to build:** Found by the user in the first real-account run (2026-10-27, second batch). Three profile surfaces differ from Discord:
- DM side panel: Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_7.png`) shows banner, avatar with status, name, username and badges, mutual servers row with icons, Member Since, View Full Profile at the bottom; kyoko (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_8.png`) shows a different layout.
- Full profile: Discord, someone else's (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_11.png`): profile theme colours and effect, avatar decoration, name, username, server tag and badges, Add Friend / Message / more, bio, Member Since (Discord and this server), Roles, Connections with details, Note, and tabs Activity / Mutual Friends / Mutual Servers on the right. Own profile (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_9.png`) adds editing (not in scope: kyoko shows the others' layout for its own user, minus Add Friend). kyoko today: `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_10.png`.
- Popout "more" (three dots) button does nothing (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_12.png`); Discord's menu (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/b_Image_13.png`): View Full Profile, Share Profile (copy link), Invite to Server (submenu), Ignore, Block, Report User Profile (open in browser), Copy User ID. Leave out Shop This Look.

**Blocked by:** 77, 81

**Status:** claimed

- [ ] DM side panel matches Discord's layout
- [ ] Full profile matches Discord's (others' layout), with the three tabs; profile theme colours (`theme_colors`) and the profile effect at least as a static gradient
- [ ] Popout more menu with the items listed (destructive ones through kyoko's existing confirmations; Ignore if the API is known, else left out and noted)
- [ ] The popout's "Message @user" quick input where Discord has it
- [ ] Captures checked by eye against the screenshots; Midnight tokens for kyoko's own surfaces
