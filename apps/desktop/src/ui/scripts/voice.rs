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

fn voice_row_values(manageable: bool, can_invite: bool) -> Vec<(&'static str, serde_json::Value)> {
	let mut rows = super::sidebar_rows();
	for row in rows.as_array_mut().into_iter().flatten() {
		if row["kind"] == "channel" {
			row["manageable"] = json!(manageable);
		}
	}
	let mut values = server_values(false);
	values.push(("sidebar.rows", rows));
	values.push((
		"sidebar.header",
		json!({
			"unread": false, "muted": false, "owner": false, "can_invite": can_invite,
			"can_manage_channels": manageable, "can_open_settings": false,
		}),
	));
	values
}

fn point_at(app: &mut super::TestApp, id: &str) {
	let bounds = app.bounds(id);
	app.ui.dispatch(zgx::input::InputEvent::PointerMove {
		x: bounds.x + bounds.width / 2.,
		y: bounds.y + bounds.height / 2.,
	});
	app.frames(2);
}

fn point_away(app: &mut super::TestApp) {
	let general = app.bounds("channel_20");
	app.ui.dispatch(zgx::input::InputEvent::PointerMove {
		x: general.x + general.width / 2.,
		y: general.y - general.height * 2.,
	});
	app.frames(2);
}

#[test]
fn a_voice_rows_hover_buttons_open_chat_invite_and_edit_without_joining() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &voice_row_values(true, true));
	assert_eq!(app.opacity("channel_buttons_21"), 0., "hidden at rest");
	assert!(
		app.find("channel_chat_20").is_none(),
		"a text row has no Open Chat"
	);
	assert!(
		app.find("channel_invite_20").is_none(),
		"a text row has no Invite to Voice"
	);
	assert!(app.find("channel_gear_20").is_some(), "a text row keeps its gear");

	point_at(&mut app, "channel_21");
	assert_eq!(app.opacity("channel_buttons_21"), 1., "shown on hover");
	for (button, tooltip) in [
		("channel_chat_21", "Open Chat"),
		("channel_invite_21", "Invite to Voice"),
		("channel_gear_21", "Edit Channel"),
	] {
		let bounds = app.bounds(button);
		assert!(
			bounds.width > 0. && bounds.height > 0.,
			"{button} has room in the row"
		);
		point_at(&mut app, button);
		assert_eq!(
			app.opacity("channel_buttons_21"),
			1.,
			"{button} keeps the buttons shown"
		);
		assert!(
			app.wait_until(|app| app.texts().iter().any(|text| text == tooltip)),
			"{button} shows the tooltip {tooltip}"
		);
	}

	app.click("channel_chat_21");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.select_channel"),
		Some(json!("21")),
		"Open Chat opens the text chat"
	);
	app.click("channel_invite_21");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.invite"),
		Some(json!("21")),
		"Invite to Voice invites to this channel"
	);
	app.click("channel_gear_21");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.edit_channel"),
		Some(json!("21")),
		"the gear edits the channel"
	);
	assert_eq!(last_call(&calls, "voice.join"), None, "no button joins voice");

	point_away(&mut app);
	assert_eq!(
		app.opacity("channel_buttons_21"),
		0.,
		"hidden once the pointer leaves"
	);
	let row = app.find("channel_21").expect("the voice row");
	app.ui.input.focus(&app.ui.scene, Some(row));
	app.frames(2);
	assert_eq!(
		app.opacity("channel_buttons_21"),
		1.,
		"shown while the row has focus"
	);
	app.ui.input.focus(&app.ui.scene, None);
	app.frames(2);
	assert_eq!(app.opacity("channel_buttons_21"), 0., "hidden when focus leaves");

	app.click("channel_21");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.join"),
		Some(json!("21")),
		"the row itself still joins"
	);
}

#[test]
fn edit_channel_and_invite_need_their_rights_on_a_voice_row() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &voice_row_values(false, false));
	point_at(&mut app, "channel_21");
	assert!(app.find("channel_chat_21").is_some(), "Open Chat needs no rights");
	assert!(
		app.find("channel_gear_21").is_none(),
		"Edit Channel needs Manage Channels"
	);
	assert!(
		app.find("channel_invite_21").is_none(),
		"Invite to Voice needs Create Invite"
	);
	assert!(
		app.find("channel_gear_20").is_none(),
		"nor does a text row get a gear"
	);
}

fn camera_call_values() -> Vec<(&'static str, serde_json::Value)> {
	let mut values = call_values();
	for (key, value) in &mut values {
		if *key == "voice.call" {
			value["tiles"][1]["video"] = json!("camera");
		}
	}
	values
}

fn reported_view(calls: &Calls) -> serde_json::Value {
	let views = last_call(calls, "voice.video_views").expect("the call view reports its video tiles");
	let views = views.as_array().expect("a list of views").clone();
	assert_eq!(views.len(), 1, "only the camera tile is reported: {views:?}");
	views[0].clone()
}

#[test]
fn a_camera_tile_shows_the_member_video_and_reports_its_size_and_focus() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(&mut app, "a camera in the call", "grace", &camera_call_values());
	assert!(
		app.find("call_video_31").is_some(),
		"the camera tile hosts the video"
	);
	assert!(app.find("call_video_30").is_none(), "an audio tile has no video");
	let thumbnail = reported_view(&calls);
	assert_eq!(thumbnail["id"], json!("31"));
	assert_eq!(thumbnail["focused"], json!(false));
	let width = thumbnail["w"].as_f64().expect("a width");
	let height = thumbnail["h"].as_f64().expect("a height");
	assert_eq!(width, f64::from(app.bounds("call_tile_31").width));
	assert!(
		(height - width * 9. / 16.).abs() < 1.,
		"the view is the 16:9 tile: {width}x{height}"
	);

	app.click("call_tile_31");
	app.frames(2);
	let focused = reported_view(&calls);
	assert_eq!(
		focused["focused"],
		json!(true),
		"a focused tile asks for full quality"
	);
	assert!(
		focused["w"].as_f64().expect("a width") > width,
		"the focused tile is larger than the thumbnail"
	);

	assert!(app.find("call_video_unavailable_31").is_none());
	shows(
		&mut app,
		"no h264 decoder",
		"grace",
		&[("voice.video_available", json!(false))],
	);
	assert!(
		app.find("call_video_unavailable_31").is_some(),
		"without Media Foundation the camera tile says video is unavailable"
	);
}
