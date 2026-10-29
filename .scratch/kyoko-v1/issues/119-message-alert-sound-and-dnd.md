# 119 — Message alert sound and Do Not Disturb (from oxidcord d0b2495)

**What to build:** Port oxidcord `d0b2495` ("Alerts come from state on message create and follow server defaults, user and channel overrides, mutes and suppression. DND silences every popup and sound. The chime is synthesized in the platform crate and played through cpal. Own status is restored from user settings and resent after reconnects."), adapted to kyoko. kyoko already has alerts and notification settings (tickets on notifications, rail and attention): first compare, and port only what kyoko lacks. User approved porting the oxidcord extras (2026-10-29).

**Status:** resolved

- [x] A message that should notify (server default, user and channel overrides, mutes, @everyone/@role suppression, as Discord) plays Discord's message sound; kyoko's chime is its own (synthesized, no Discord audio asset)
- [x] Do Not Disturb silences every popup and sound; own status restored from user settings and resent after reconnect (check whether kyoko already does)
- [x] Session tests against mockcord; no real Windows notifications or sounds in tests (sound behind a seam)
- [x] Commit bodies end `Ported from oxidcord d0b2495...` (full sha)
- [x] Report what kyoko already had, what was ported, what was skipped

## Answer

Merged `ea4db0d`, `102cd95`, `c68340e` (lane C, worker `d20facd5`), verified by the orchestrator: all done-means steps green; tls-capture chrome-156 0/0; `parity mockup` settings-notifications 5.7% differ (new "Message" row under Sounds reads right), guild-notifications 17.2% (layout gaps pre-date this).

- Already in kyoko (not ported): alert rules from state (40, `State::alert_for`, session `task/alerts.rs`), DND dropping alerts, own status from settings proto field 11 (30; identify says "unknown"), status in the user panel, clickable toasts. Skipped: DND check on plugin notify (kyoko plugins can't post notifications).
- Ported: kyoko's own chime (`audio::message_chime`, two notes ~0.4 s, synthesized) through `audio::Cue` (`crates/audio/src/cue.rs`: `Cue::once` / `Cue::looping`, `play`/`stop`/`is_playing`, opens the default device only while playing, own thread). Toasts are silent; the chime plays once per batch for accepted alerts, independent of the desktop-notifications switch, silenced by DND; seam `bridge::raise_alerts(AlertOutlet)`, desktop outlet `notify::Desk`. Settings row "Message" under Sounds. `Core::restore_presence`: after a fresh READY, op 3 again only while away or while a status write is pending (Discord applies the saved status after "unknown"). `State::guild_notification_level`: the server dialog shows the effective level (missing/unknown default stays "only mentions").
- Tests: bridge `alerts::alerts_chime_once_and_toast_each_message_as_the_settings_say`; session `alerts::do_not_disturb_from_the_saved_settings_silences_every_alert`, `status::an_unsaved_status_survives_a_fresh_session_and_goes_out_again`, `status::being_away_goes_out_again_after_a_fresh_session`, `status::a_fresh_session_leaves_a_saved_status_to_discord`.
- Docs: `docs/desktop.md` (Notifications, Cue), `docs/fingerprint.md` (presence after a fresh READY). 108's ringtone should use `Cue::looping`.
- New load flake: `voice_settings::high_quality_noise_suppression_is_the_default_and_cleans_noisy_speech` (SI-SDR short under load), added to rules.md.
