use serde_json::json;

use super::{Calls, WIDE, last_call, open_channel_menu, recording_app, server_values, shows};

pub(super) fn connected_panel() -> serde_json::Value {
	json!({
		"open": true, "channel_id": "21", "status": "Voice Connected", "connected": true, "place": "Lounge / Kyoko",
	})
}

pub(super) fn call_values() -> Vec<(&'static str, serde_json::Value)> {
	let tile = |id: &str, name: &str, me: bool, muted: bool| {
		json!({
			"key": format!("t{id}"), "id": id, "name": name, "avatar": "", "me": me, "muted": muted,
			"deafened": false, "video": "", "live": false, "volume": 100, "local_muted": false,
		})
	};
	vec![
		("voice.panel", connected_panel()),
		(
			"voice.call",
			json!({
				"open": true, "channel_id": "21", "name": "Lounge", "connected": true,
				"tiles": [tile("30", "ada", true, false), tile("31", "grace", false, true)],
			}),
		),
		("voice.speaking", json!({ "31": true })),
	]
}

pub(super) fn closed_call() -> serde_json::Value {
	json!({ "open": false, "channel_id": "", "name": "", "connected": false, "tiles": [] })
}

#[test]
fn the_call_view_shows_the_tiles_and_its_controls_and_menu_reach_the_host() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(&mut app, "the call view", "grace", &call_values());
	for tile in ["call_tile_30", "call_tile_31"] {
		let bounds = app.bounds(tile);
		assert!(bounds.width > 0. && bounds.height > 0., "{tile} is laid out");
	}
	assert!(app.find("call_view").is_some(), "the call view takes the page");
	app.click("call_camera");
	app.click("call_screen");
	app.frames(2);
	assert!(
		calls.borrow().is_empty(),
		"camera and screen share wait for their tickets"
	);
	app.click("call_mute");
	app.click("call_deafen");
	app.click("call_open_chat");
	app.click("call_disconnect");
	app.frames(2);
	let names: Vec<String> = calls.borrow().iter().map(|(name, _)| name.clone()).collect();
	for wanted in [
		"voice.toggle_mute",
		"voice.toggle_deafen",
		"voice.open_chat",
		"voice.leave",
	] {
		assert!(
			names.iter().any(|name| name == wanted),
			"{wanted} was not called: {names:?}"
		);
	}

	open_channel_menu(&mut app, "call_tile_30");
	assert!(
		app.find("call_menu_mute").is_none(),
		"your own tile has no volume menu"
	);
	open_channel_menu(&mut app, "call_tile_31");
	app.click("call_menu_mute");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.local_mute"),
		Some(json!({ "id": "31", "muted": true }))
	);
	open_channel_menu(&mut app, "call_tile_31");
	app.click("call_menu_volume");
	app.frames(2);
	let volume = last_call(&calls, "voice.user_volume").expect("the slider sets the user volume");
	assert_eq!(volume["id"], json!("31"));
	let percent = volume["volume"].as_f64().expect("a volume");
	assert!(
		(90. ..=110.).contains(&percent),
		"the middle of the slider is about 100%: {percent}"
	);
}

#[test]
fn a_speaking_member_gets_a_ring_in_the_sidebar() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	assert!(app.find("voice_ring_30").is_none(), "nobody speaks yet");
	shows(
		&mut app,
		"a speaking member",
		"general",
		&[("voice.speaking", json!({ "30": true }))],
	);
	assert!(
		app.find("voice_ring_30").is_some(),
		"the speaker's avatar is ringed"
	);
	shows(
		&mut app,
		"the member falling quiet",
		"general",
		&[("voice.speaking", json!({}))],
	);
	assert!(
		app.find("voice_ring_30").is_none(),
		"the ring goes with the speech"
	);
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
