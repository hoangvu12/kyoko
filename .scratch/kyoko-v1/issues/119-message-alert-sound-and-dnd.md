# 119 — Message alert sound and Do Not Disturb (from oxidcord d0b2495)

**What to build:** Port oxidcord `d0b2495` ("Alerts come from state on message create and follow server defaults, user and channel overrides, mutes and suppression. DND silences every popup and sound. The chime is synthesized in the platform crate and played through cpal. Own status is restored from user settings and resent after reconnects."), adapted to kyoko. kyoko already has alerts and notification settings (tickets on notifications, rail and attention): first compare, and port only what kyoko lacks. User approved porting the oxidcord extras (2026-10-29).

**Status:** claimed

- [ ] A message that should notify (server default, user and channel overrides, mutes, @everyone/@role suppression, as Discord) plays Discord's message sound; kyoko's chime is its own (synthesized, no Discord audio asset)
- [ ] Do Not Disturb silences every popup and sound; own status restored from user settings and resent after reconnect (check whether kyoko already does)
- [ ] Session tests against mockcord; no real Windows notifications or sounds in tests (sound behind a seam)
- [ ] Commit bodies end `Ported from oxidcord d0b2495...` (full sha)
- [ ] Report what kyoko already had, what was ported, what was skipped