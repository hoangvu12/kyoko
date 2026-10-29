use serde_json::json;
use zgx::input::{InputEvent, Key, PointerButton};
use zgx_script::Value;
use zgx_script::testing::TestApp;

use super::{Calls, WIDE, last_call, recording_app, shows};
use crate::ui::voice_settings::listen_for_side_buttons;

const LEVEL: f64 = 0.6;
const TALL: f32 = 2400.;

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
			(
				"voice.cameras",
				json!([{ "id": "camera-1", "name": "Desk Camera" }]),
			),
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
		"High Quality",
		"Low CPU",
		"Off",
		"Echo Cancellation",
		"Video Settings",
		"Camera",
		"Test Video",
	] {
		assert!(texts.iter().any(|text| text == shown), "{shown} is not shown");
	}
	let high_quality = app
		.find("voice_noise_suppression_1")
		.expect("the High Quality choice");
	assert_eq!(
		app.ui
			.semantics
			.borrow()
			.get(high_quality)
			.and_then(|semantic| semantic.checked),
		Some(true),
		"High Quality is chosen by default"
	);
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

	app.click("voice_camera_test");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.camera_preview"), Some(json!(true)));
	shows(
		&mut app,
		"a running camera preview",
		"Stop Video",
		&[("voice.camera_preview", json!(true))],
	);
	assert!(
		app.find("voice_camera_preview_image").is_some(),
		"the preview shows the camera"
	);
	let preview = app.bounds("voice_camera_preview");
	assert!(
		(preview.height - preview.width * 9. / 16.).abs() < 1.,
		"the preview is 16:9"
	);
	app.click("voice_camera_stop");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.camera_preview"), Some(json!(false)));
	shows(
		&mut app,
		"a stopped camera preview",
		"Test Video",
		&[("voice.camera_preview", json!(false))],
	);
	app.click("voice_camera_device");
	app.frames(2);
	app.click("voice_camera_device_2");
	app.frames(2);

	app.click("voice_input_device");
	app.frames(2);
	app.click("voice_input_device_3");
	app.frames(2);
	app.click("voice_input_mode_2");
	app.click("voice_auto_sensitivity");
	app.click("voice_noise_suppression_2");
	app.click("voice_echo_cancellation");
	app.frames(2);
	let written = setting_calls(&calls);
	for wanted in [
		json!({ "key": "voice.input_device", "value": "mic-2" }),
		json!({ "key": "voice.push_to_talk", "value": true }),
		json!({ "key": "voice.auto_sensitivity", "value": false }),
		json!({ "key": "voice.noise_suppression", "value": "low_cpu" }),
		json!({ "key": "voice.camera_device", "value": "camera-1" }),
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
		"Push to Talk Release Delay",
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

fn press_side_button(app: &mut TestApp, id: &str, number: u16) {
	let bounds = app.bounds(id);
	let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
	app.ui.dispatch(InputEvent::PointerDown {
		x,
		y,
		button: PointerButton::Other(number),
	});
	app.ui.dispatch(InputEvent::PointerUp {
		x,
		y,
		button: PointerButton::Other(number),
	});
	app.settle();
	app.frames(2);
}

fn talk_bindings(calls: &Calls) -> Vec<serde_json::Value> {
	setting_calls(calls)
		.into_iter()
		.filter(|call| call["key"] == "keybinds.push_to_talk")
		.map(|call| call["value"].clone())
		.collect()
}

#[test]
fn mouse_4_records_as_the_push_to_talk_shortcut_and_the_release_delay_shows() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, TALL);
	shows(
		&mut app,
		"push to talk",
		"Push to Talk Release Delay",
		&[
			("login.screen", json!("main")),
			(
				"settings.values",
				voice_values(|settings| settings.voice.push_to_talk = true),
			),
			("settings.page", json!("voice")),
		],
	);
	let texts = app.texts();
	for shown in [
		"20 ms",
		"A game run as administrator needs Kyoko run as administrator too, or the shortcut won't work while the game is focused.",
	] {
		assert!(texts.iter().any(|text| text == shown), "{shown} is not shown");
	}
	assert!(
		app.bounds("voice_release_delay").width > 0.,
		"the delay slider is laid out"
	);

	press_side_button(&mut app, "voice_push_to_talk_record", 5);
	assert!(
		talk_bindings(&calls).is_empty(),
		"a side button does nothing until recording starts"
	);

	app.click("voice_push_to_talk_record_button");
	app.frames(2);
	assert!(
		app.texts()
			.iter()
			.any(|text| text == "Press a key or side button..."),
		"the recorder waits for a key or a side button"
	);
	assert_eq!(
		last_call(&calls, "keybinds.side_buttons"),
		Some(json!(true)),
		"recording asks the host for side buttons"
	);
	let pressed = app.host.shared("keybinds.side_button", Value::Nil);
	let listener = listen_for_side_buttons(&app.ui, pressed);
	press_side_button(&mut app, "voice_push_to_talk_record", 4);
	assert_eq!(talk_bindings(&calls), [json!("Mouse4")]);
	assert!(
		app.texts()
			.iter()
			.all(|text| text != "Press a key or side button..."),
		"recording ends with the side button"
	);
	assert_eq!(
		last_call(&calls, "keybinds.side_buttons"),
		Some(json!(false)),
		"the host stops listening once recording ends"
	);
	drop(listener);

	app.click("voice_push_to_talk_record_button");
	app.frames(2);
	app.key(Key::Function(13));
	app.frames(2);
	assert_eq!(
		talk_bindings(&calls),
		[json!("Mouse4"), json!("F13")],
		"keys still record after a side button"
	);

	shows(
		&mut app,
		"a mouse binding",
		"MOUSE 4",
		&[(
			"settings.values",
			voice_values(|settings| {
				settings.voice.push_to_talk = true;
				settings.keybinds.push_to_talk = Some("Mouse4".to_owned());
			}),
		)],
	);
}
