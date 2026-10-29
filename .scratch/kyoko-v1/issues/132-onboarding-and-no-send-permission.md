# 132 — Unfinished onboarding and channels you can't send in

**What to build:** From the user's real run 4 (2026-10-30), image-19 in `C:/Users/HP MEDIA/Desktop/nguyenvu/.orchestrator/real-run/screens/run4/` (real people's names: look, never copy). In a server whose onboarding the user hasn't finished, kyoko showed an "Onboarding: missing permissions" error toast (the log has `rest call failed error=forbidden what="Onboarding"`), the banner "This server's onboarding isn't finished...", and the composer said "Waiting for connection..." (that part was the gateway reconnecting, ticket 124). The user typed in a channel where they can't send, and the composer let them.

- Find which request got 403 and whether the web client makes it in that situation (docs.discord.food; 35's community flows). Don't show an error toast for an expected 403; show Discord's own state instead.
- The composer follows Discord where you lack Send Messages (or the channel is locked by onboarding, timeout, or slowmode bypass rules already in 11/117): it doesn't accept typing and shows "You do not have permission to send messages in this channel." (Discord's wording and look), with no request sent.

**Blocked by:** 124 (same run; read its Answer)

**Status:** ready-for-agent

- [ ] The 403 explained; no error toast for it; the onboarding banner matches Discord's behaviour
- [ ] Composer refuses typing without Send Messages and shows Discord's message; session test proves no send request and a bridge/UI test proves the composer state
- [ ] Captures checked by eye against image-19
