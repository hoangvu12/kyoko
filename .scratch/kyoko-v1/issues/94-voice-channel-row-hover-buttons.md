# 94 — Voice channel rows: Discord's hover buttons

**What to build:** From the user after the first real-account run (2026-10-28). Hovering a voice channel row in Discord (`C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/c_Image_1.png`) shows three buttons at the right: Open Chat (the voice channel's text chat), Invite to Voice (invite dialog for that channel), Edit Channel (gear, only with Manage Channels). Clicking the row itself joins voice (that's the voice tickets from the voice plan; until they land, a row click keeps today's behaviour). Adapt the look to Midnight, don't copy Discord's colours.

**Blocked by:** none

**Status:** claimed

- [ ] Voice rows show Open Chat, Invite to Voice and Edit Channel on hover (and while focused), with tooltips; Edit Channel only with Manage Channels; text rows keep their own hover buttons
- [ ] Open Chat opens the voice channel's text chat; Invite opens kyoko's invite dialog for that channel (v1 35); Edit Channel opens channel settings (95; until 95 lands, the existing server settings page or nothing, but never a dead button)
- [ ] In-process UI test for the buttons; capture checked by eye
