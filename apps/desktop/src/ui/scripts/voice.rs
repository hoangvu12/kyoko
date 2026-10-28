use serde_json::json;

use super::{Calls, WIDE, last_call, recording_app, server_values, shows};

pub(super) fn connected_panel() -> serde_json::Value {
	json!({
		"open": true, "channel_id": "21", "status": "Voice Connected", "connected": true, "place": "Lounge / Kyoko",
	})
}

#[test]
fn a_voice_row_joins_and_the_voice_panel_shows_the_channel_and_disconnects() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	assert!(
		!app.texts().iter().any(|text| text == "Voice Connected"),
		"no voice panel before joining"
	);
	app.click("channel_21");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.join"),
		Some(json!("21")),
		"clicking a voice row joins it"
	);
	assert_eq!(
		last_call(&calls, "sidebar.select_channel"),
		None,
		"a voice row does not open its text chat on the first click"
	);
	app.click("channel_20");
	app.frames(2);
	assert_eq!(last_call(&calls, "sidebar.select_channel"), Some(json!("20")));

	shows(
		&mut app,
		"the voice panel",
		"Lounge / Kyoko",
		&[("voice.panel", connected_panel())],
	);
	assert!(app.texts().iter().any(|text| text == "Voice Connected"));
	app.click("voice_panel_place");
	app.frames(2);
	assert!(
		calls.borrow().iter().any(|(name, _)| name == "voice.open_call"),
		"the channel line opens the call"
	);
	app.click("user_panel_mute");
	app.frames(2);
	app.click("user_panel_deafen");
	app.frames(2);
	let names: Vec<String> = calls.borrow().iter().map(|(name, _)| name.clone()).collect();
	assert!(names.iter().any(|name| name == "voice.toggle_mute"));
	assert!(names.iter().any(|name| name == "voice.toggle_deafen"));

	app.click("voice_panel_disconnect");
	app.frames(2);
	assert!(
		calls.borrow().iter().any(|(name, _)| name == "voice.leave"),
		"Disconnect leaves voice"
	);
	shows(
		&mut app,
		"the sidebar after leaving",
		"general",
		&[(
			"voice.panel",
			json!({ "open": false, "channel_id": "", "status": "", "connected": false, "place": "" }),
		)],
	);
	assert!(
		!app.texts().iter().any(|text| text == "Lounge / Kyoko"),
		"the voice panel closes once voice is left"
	);
}
