use serde_json::json;

use super::{Calls, WIDE, last_call, recording_app, shows};

const LEVEL: f64 = 0.6;
const TALL: f32 = 1600.;

fn voice_values(change: impl FnOnce(&mut store::Settings)) -> serde_json::Value {
	let mut settings = store::Settings::default();
	change(&mut settings);
	serde_json::Value::Object(
		bridge::setting_values(&settings)
			.into_iter()
			.map(|(key, value)| (key.to_owned(), value))
			.collect(),
	)
}

fn devices() -> serde_json::Value {
	json!({
		"inputs": [
			{ "id": "mic-1", "name": "Desk Microphone" },
			{ "id": "mic-2", "name": "Headset Microphone" },
		],
		"outputs": [{ "id": "speakers-1", "name": "Desk Speakers" }],
	})
}

fn setting_calls(calls: &Calls) -> Vec<serde_json::Value> {
	calls
		.borrow()
		.iter()
		.filter(|(name, _)| name == "settings.set")
		.filter_map(|(_, argument)| serde_json::from_str(argument).ok())
		.collect()
}

#[test]
fn the_voice_page_tests_the_mic_and_writes_every_voice_setting() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, TALL);
	shows(
		&mut app,
		"the voice and video page",
		"Let's Check",
		&[
			("login.screen", json!("main")),
			("settings.values", voice_values(|_| {})),
			("voice.devices", devices()),
			("settings.page", json!("voice")),
		],
	);
	let texts = app.texts();
	for shown in [
		"Input Device",
		"Output Device",
		"Input Volume",
		"Output Volume",
		"Voice Activity",
		"Push to Talk",
		"Automatically determine input sensitivity",
		"Noise Suppression",
		"Echo Cancellation",
	] {
		assert!(texts.iter().any(|text| text == shown), "{shown} is not shown");
	}
	assert!(
		app.find("voice_sensitivity").is_none(),
		"automatic sensitivity hides the threshold"
	);
	assert!(
		app.find("voice_push_to_talk_record").is_none(),
		"voice activity hides the push to talk shortcut"
	);

	app.click("voice_mic_test");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.mic_test"), Some(json!(true)));
	shows(
		&mut app,
		"a running mic test",
		"Stop Testing",
		&[("voice.mic_test", json!({ "running": true, "level": LEVEL }))],
	);
	let meter = app.bounds("voice_mic_meter");
	let level = app.bounds("voice_mic_meter_level");
	assert!(meter.width > 0., "the meter is laid out");
	assert!(
		(f64::from(level.width / meter.width) - LEVEL).abs() < 0.05,
		"the meter fills to the level: {} of {}",
		level.width,
		meter.width
	);
	app.click("voice_mic_test");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.mic_test"), Some(json!(false)));

	app.click("voice_input_device");
	app.frames(2);
	app.click("voice_input_device_3");
	app.frames(2);
	app.click("voice_input_mode_2");
	app.click("voice_auto_sensitivity");
	app.click("voice_noise_suppression");
	app.click("voice_echo_cancellation");
	app.frames(2);
	let written = setting_calls(&calls);
	for wanted in [
		json!({ "key": "voice.input_device", "value": "mic-2" }),
		json!({ "key": "voice.push_to_talk", "value": true }),
		json!({ "key": "voice.auto_sensitivity", "value": false }),
		json!({ "key": "voice.noise_suppression", "value": true }),
	] {
		assert!(written.contains(&wanted), "{wanted} was not written: {written:?}");
	}
	assert!(
		written
			.iter()
			.all(|call| !call["key"].as_str().unwrap_or_default().contains("echo")),
		"echo cancellation is not available"
	);

	shows(
		&mut app,
		"manual sensitivity",
		"-40 dB",
		&[(
			"settings.values",
			voice_values(|settings| {
				settings.voice.auto_sensitivity = false;
				settings.voice.sensitivity_db = -40;
			}),
		)],
	);
	let track = app.bounds("voice_sensitivity");
	assert!(track.width > 0., "the threshold slider is laid out");

	shows(
		&mut app,
		"push to talk",
		"Hold the shortcut to talk. It works while Kyoko is in the background.",
		&[(
			"settings.values",
			voice_values(|settings| settings.voice.push_to_talk = true),
		)],
	);
	assert!(app.find("voice_push_to_talk_record").is_some());
	assert!(
		app.find("voice_auto_sensitivity").is_none(),
		"push to talk has no input sensitivity"
	);
}
