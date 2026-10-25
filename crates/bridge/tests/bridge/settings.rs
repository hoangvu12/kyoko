use bridge::{Applies, SettingKey, change_setting};
use serde_json::{Value, json};
use store::{Paths, Settings};

use crate::harness::Running;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_safety_locks_setting_is_read_at_the_next_session_start() {
	let mut running = Running::start("settings-locks").await;
	running
		.until_state("the locks to be on", |state| state.safety_locks())
		.await;
	let paths = Paths::at(&running.scratch).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	let changed = change_setting(&mut settings, "safety_locks", &Value::Bool(false));
	assert_eq!(changed, Some(SettingKey::SafetyLocks));
	assert_eq!(SettingKey::SafetyLocks.applies(), Applies::AfterRestart);
	settings.save(&paths).expect("settings save");
	assert!(
		running.session().state().read().safety_locks(),
		"the running session keeps the setting it started with"
	);
	running.restart().await;
	running
		.until_state("the locks to be off", |state| !state.safety_locks())
		.await;
	assert!(!Settings::load(&paths).safety_locks, "the change persisted");
	running.stop().await;
}

#[test]
fn unknown_keys_and_bad_values_change_nothing() {
	let mut settings = Settings::default();
	let before = settings.clone();
	assert_eq!(
		change_setting(&mut settings, "telemetry.enabled", &json!(true)),
		None
	);
	assert_eq!(
		change_setting(&mut settings, "chat.inline_media", &json!("yes")),
		None
	);
	assert_eq!(
		change_setting(&mut settings, "media.animation", &json!("sometimes")),
		None
	);
	assert_eq!(change_setting(&mut settings, "theme", &json!("../../evil")), None);
	assert_eq!(
		change_setting(&mut settings, "chat.inline_media", &json!(true)),
		None,
		"unchanged"
	);
	assert_eq!(settings, before);
	assert_eq!(
		change_setting(&mut settings, "accessibility.saturation", &json!(250)),
		None
	);
	assert_eq!(
		settings.accessibility.saturation, 100,
		"clamped to the top, which it already was"
	);
	assert_eq!(
		change_setting(&mut settings, "accessibility.saturation", &json!(-5)),
		Some(SettingKey::Saturation)
	);
	assert_eq!(settings.accessibility.saturation, 0);
	assert_eq!(
		change_setting(
			&mut settings,
			"trusted_link_hosts",
			&json!([" Example.com ", "example.com", 5])
		),
		Some(SettingKey::TrustedLinkHosts)
	);
	assert_eq!(settings.trusted_link_hosts, vec!["example.com".to_owned()]);
}

#[test]
fn every_key_reads_back_what_it_wrote() {
	let settings = Settings::default();
	for (name, value) in bridge::setting_values(&settings) {
		let mut copy = settings.clone();
		assert_eq!(
			change_setting(&mut copy, name, &value),
			None,
			"{name} round-trips"
		);
		assert_eq!(SettingKey::named(name).map(SettingKey::name), Some(name));
	}
}
