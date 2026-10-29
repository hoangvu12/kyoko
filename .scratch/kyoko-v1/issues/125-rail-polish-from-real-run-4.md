# 125 — Server rail polish: hovers, tooltips, cursors, voice badge, logo

**What to build:** From the user's real run 4 (2026-10-30). Screenshots in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy into the repo). Discord's look is the reference for behaviour; keep Midnight's colours.

- image-01: the "Read All Notifications" rail button's icon turns black on hover. It must keep a readable icon colour on hover like the other rail buttons.
- image-03: a hover background shows behind server icons on the rail. Remove it (Discord only changes the icon's shape and the pill). Rail tooltips show immediately on hover, not after a delay.
- image-04 (Discord) vs image-05 (kyoko): the Discover button's hover should match Discord's (the icon tile fills, no extra background). Every rail button (home, Read All, add server, Discover, plugin rail buttons) uses the same tooltip as the server icons; some still use the old tooltip style.
- The Direct Messages (home) button and the Read All button show the pointer cursor, like every other clickable rail item.
- image-08 (Discord) vs image-09 (kyoko): the voice badge on a server icon turns green when you are connected to a voice channel in that server; kyoko's badge is hard to see. Match Discord's badge (size, contrast, green while you are in that server's voice; the neutral badge when only others are in voice).
- Round kyoko's logo a little on the rail's home button (it has sharp corners now).

**Blocked by:** none

**Status:** claimed (lane B)

- [ ] Read All's icon keeps its colour on hover; no hover background behind server icons; rail tooltips appear without delay
- [ ] Discover's hover matches Discord; every rail button uses the server-icon tooltip
- [ ] Home and Read All show the pointer cursor
- [ ] Voice badge is clearly visible and green when you are in that server's voice (mockcord can show both states)
- [ ] Home logo has slightly rounded corners
- [ ] In-process UI tests where behaviour changed (cursor, tooltip delay, badge state); kyoko-only captures of the rail (hover states included) checked by eye against the screenshots
