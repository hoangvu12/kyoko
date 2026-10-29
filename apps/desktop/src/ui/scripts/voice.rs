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
	app.click("call_screen");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.go_live"), Some(json!(true)));
	calls.borrow_mut().clear();
	app.click("call_camera");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.toggle_camera"), Some(json!(true)));
	app.click("call_mute");
	app.click("call_deafen");
	app.click("call_open_chat");
	app.click("call_disconnect");
	app.frames(2);
	let names: Vec<String> = calls.borrow().iter().map(|(name, _)| name.clone()).collect();
	for wanted in [
		"voice.toggle_camera",
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

fn stream_call_values(watching: bool) -> Vec<(&'static str, serde_json::Value)> {
	let mut values = call_values();
	for (key, value) in &mut values {
		if *key == "voice.call" {
			value["tiles"][1]["live"] = json!(true);
			let stream = json!({
				"key": "s31", "id": "31", "name": "grace", "avatar": "", "me": false, "muted": false,
				"deafened": false, "video": "stream", "live": true, "volume": 100, "local_muted": false,
				"stream": true, "stream_key": "guild:20:21:31", "watching": watching,
			});
			value["tiles"]
				.as_array_mut()
				.expect("the tiles")
				.insert(1, stream);
		}
	}
	values
}

#[test]
fn a_stream_tile_watches_focuses_reports_its_view_and_stops_watching() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(
		&mut app,
		"a stream in the call",
		"Watch Stream",
		&stream_call_values(false),
	);
	assert!(
		app.find("call_stream_31").is_some(),
		"the streamer gets a stream tile"
	);
	assert!(app.find("call_tile_31").is_some(), "and keeps their own tile");
	assert!(
		app.find("call_stream_video_31").is_none(),
		"nothing streams before Watch Stream"
	);
	assert!(
		calls
			.borrow()
			.iter()
			.all(|(name, _)| name != "voice.watch_stream")
	);
	app.click("call_watch_31");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.watch_stream"),
		Some(json!({ "key": "guild:20:21:31" }))
	);

	shows(
		&mut app,
		"watching the stream",
		"Stop Watching",
		&stream_call_values(true),
	);
	assert!(
		app.find("call_stream_video_31").is_some(),
		"the stream video shows"
	);
	assert!(
		app.find("call_tile_30").is_none(),
		"a watched stream takes the stage"
	);
	let views = last_call(&calls, "voice.stream_views").expect("the stream view is reported");
	let views = views.as_array().expect("a list of views");
	assert_eq!(views.len(), 1);
	assert_eq!(views[0]["id"], json!("31"));
	assert_eq!(views[0]["focused"], json!(true));

	app.click("call_stop_watching_31");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.stop_watching"),
		Some(json!({ "key": "guild:20:21:31" }))
	);
}

fn stream_view(calls: &Calls) -> serde_json::Value {
	let views = last_call(calls, "voice.stream_views").expect("the stream view is reported");
	let views = views.as_array().expect("a list of views").clone();
	assert_eq!(views.len(), 1, "one watched stream: {views:?}");
	views[0].clone()
}

fn open_full_window(app: &mut super::TestApp) {
	point_at(app, "call_stream_31");
	app.click("call_stream_full_31");
	app.frames(3);
	assert!(
		app.find("stream_viewer").is_some(),
		"the full-window view opens from the focused stream tile"
	);
}

#[test]
fn the_full_window_stream_view_shows_the_stream_sets_its_volume_and_closes() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(
		&mut app,
		"watching the stream",
		"Stop Watching",
		&stream_call_values(true),
	);
	assert!(app.find("stream_viewer").is_none(), "closed until asked for");
	let tile_width = stream_view(&calls)["w"].as_f64().expect("a width");

	open_full_window(&mut app);
	let viewer = app.bounds("stream_viewer");
	assert_eq!(
		(viewer.width, viewer.height),
		WIDE,
		"the stream view fills the window"
	);
	assert!(
		app.find("stream_viewer_video_31").is_some(),
		"it shows the stream"
	);
	assert!(app.texts().iter().any(|text| text == "Stream Volume"));
	let full = stream_view(&calls);
	assert_eq!(full["focused"], json!(true));
	assert_eq!(full["w"].as_f64(), Some(f64::from(WIDE.0)));
	assert!(
		f64::from(WIDE.0) > tile_width,
		"the window asks for a larger view"
	);

	app.click("stream_viewer_volume");
	app.frames(2);
	let volume = last_call(&calls, "voice.stream_volume").expect("the slider sets the stream volume");
	assert_eq!(volume["id"], json!("31"));
	let percent = volume["volume"].as_f64().expect("a volume");
	assert!(
		(90. ..=110.).contains(&percent),
		"the middle of the slider is about 100%: {percent}"
	);
	assert_eq!(
		last_call(&calls, "voice.user_volume"),
		None,
		"the stream volume is not the voice volume"
	);

	app.key(zgx::input::Key::Escape);
	app.frames(3);
	assert!(app.find("stream_viewer").is_none(), "Escape returns to the call");
	assert!(app.find("call_stream_video_31").is_some());
	assert_eq!(stream_view(&calls)["w"].as_f64(), Some(tile_width));

	open_full_window(&mut app);
	app.click("stream_viewer_exit");
	app.frames(3);
	assert!(
		app.find("stream_viewer").is_none(),
		"the exit button returns to the call"
	);

	open_full_window(&mut app);
	app.click("stream_viewer_stop");
	app.frames(3);
	assert_eq!(
		last_call(&calls, "voice.stop_watching"),
		Some(json!({ "key": "guild:20:21:31" }))
	);
	assert!(
		app.find("stream_viewer").is_none(),
		"Stop Watching closes the view"
	);
}

fn own_camera_values() -> Vec<(&'static str, serde_json::Value)> {
	let mut values = call_values();
	for (key, value) in &mut values {
		if *key == "voice.call" {
			value["tiles"][0]["video"] = json!("camera");
		}
	}
	values.push((
		"voice.controls",
		json!({ "muted": false, "deafened": false, "server_muted": false, "server_deafened": false, "camera": true }),
	));
	values
}

#[test]
fn your_camera_shows_on_your_own_tile_and_the_button_turns_it_off() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(&mut app, "your camera in the call", "ada", &own_camera_values());
	assert!(
		app.find("call_video_30").is_some(),
		"your own tile shows your camera"
	);
	let view = reported_view(&calls);
	assert_eq!(
		view["id"],
		json!("30"),
		"your own preview is sized like any camera tile"
	);
	app.click("call_camera");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.toggle_camera"), Some(json!(true)));

	calls.borrow_mut().clear();
	shows(
		&mut app,
		"no h264 codec",
		"ada",
		&[("voice.video_available", json!(false))],
	);
	app.click("call_camera");
	app.frames(2);
	assert!(
		last_call(&calls, "voice.toggle_camera").is_none(),
		"without Media Foundation the camera button does nothing"
	);
}

pub(super) fn share_sources() -> serde_json::Value {
	json!({
		"open": true,
		"loading": false,
		"items": [
			{ "id": "screen-1", "kind": "screen", "name": "Screen 1", "size": "1920x1080" },
			{ "id": "window-2", "kind": "window", "name": "Mock Browser", "size": "1280x800" },
			{ "id": "window-3", "kind": "window", "name": "Mock Notes", "size": "960x720" },
		],
	})
}

fn screen_preview(id: &str, name: &str, resolution: &str, fps: u32, running: bool) -> serde_json::Value {
	let (width, height) = if running { (1280, 720) } else { (0, 0) };
	json!({
		"id": id, "name": name, "resolution": resolution, "fps": fps,
		"width": width, "height": height, "running": running,
	})
}

fn closed_picker() -> serde_json::Value {
	json!({ "open": false, "loading": false, "items": [] })
}

#[test]
fn the_go_live_picker_lists_screens_and_windows_previews_one_and_closes() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(&mut app, "the call view", "grace", &call_values());
	assert!(app.find("go_live_close").is_none(), "closed until asked for");

	app.click("call_screen");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.go_live"), Some(json!(true)));
	shows(
		&mut app,
		"the picker while it looks",
		"Looking for things to share...",
		&[(
			"voice.screens",
			json!({ "open": true, "loading": true, "items": [] }),
		)],
	);
	shows(
		&mut app,
		"the picker's applications",
		"Mock Browser",
		&[("voice.screens", share_sources())],
	);
	for shown in ["Screen Share", "Applications", "Screens", "Mock Notes"] {
		assert!(
			app.texts().iter().any(|text| text == shown),
			"{shown} is not shown"
		);
	}
	assert!(
		app.find("go_live_thumb_window-2").is_some(),
		"a window shows its thumbnail"
	);
	assert!(
		app.find("go_live_source_screen-1").is_none(),
		"screens wait for their tab"
	);
	let tile = app.bounds("go_live_source_window-2");
	let thumbnail = app.bounds("go_live_thumb_window-2");
	assert!(tile.width > 0., "the tile is laid out");
	assert!(
		(thumbnail.height - thumbnail.width * 9. / 16.).abs() < 1.,
		"thumbnails are 16:9"
	);

	app.click("go_live_tab_screen");
	app.frames(2);
	assert!(
		app.find("go_live_source_screen-1").is_some(),
		"the Screens tab lists the screens"
	);
	assert!(app.find("go_live_source_window-2").is_none());

	app.click("go_live_source_screen-1");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.screen_preview"),
		Some(json!({ "id": "screen-1", "resolution": "720", "fps": 30 }))
	);
	shows(
		&mut app,
		"a preview starting",
		"Starting preview...",
		&[(
			"voice.screen_preview",
			screen_preview("screen-1", "Screen 1", "720", 30, false),
		)],
	);
	shows(
		&mut app,
		"the preview",
		"1280x720 at 30 FPS",
		&[(
			"voice.screen_preview",
			screen_preview("screen-1", "Screen 1", "720", 30, true),
		)],
	);
	for shown in [
		"Stream Quality",
		"Resolution",
		"Frame Rate",
		"720p",
		"1080p",
		"Source",
		"60 FPS",
		"Go Live",
	] {
		assert!(
			app.texts().iter().any(|text| text == shown),
			"{shown} is not shown"
		);
	}
	assert!(
		app.find("go_live_preview_image").is_some(),
		"the preview shows the screen"
	);
	let preview = app.bounds("go_live_preview");
	assert!(
		(preview.height - preview.width * 9. / 16.).abs() < 1.,
		"the preview is 16:9"
	);

	app.click("go_live_resolution_1080");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.screen_preview"),
		Some(json!({ "id": "screen-1", "resolution": "1080", "fps": 30 }))
	);
	app.click("go_live_fps_60");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.screen_preview"),
		Some(json!({ "id": "screen-1", "resolution": "720", "fps": 60 }))
	);
	calls.borrow_mut().clear();
	app.click("go_live_start");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.go_live_start"),
		Some(json!({ "id": "screen-1", "resolution": "720", "fps": 30 })),
		"Go Live goes live with the previewed source and quality"
	);

	app.click("go_live_change");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.screen_preview"), Some(json!(false)));
	shows(
		&mut app,
		"the list after Change",
		"Screen 1",
		&[("voice.screen_preview", screen_preview("", "", "720", 30, false))],
	);
	assert!(app.find("go_live_preview").is_none());

	app.click("go_live_source_screen-1");
	app.frames(2);
	shows(
		&mut app,
		"a second preview",
		"1280x720 at 30 FPS",
		&[(
			"voice.screen_preview",
			screen_preview("screen-1", "Screen 1", "720", 30, true),
		)],
	);
	shows(
		&mut app,
		"the list after the shared screen closed",
		"Screen 1",
		&[("voice.screen_preview", screen_preview("", "", "720", 30, false))],
	);
	assert!(
		app.find("go_live_preview").is_none(),
		"a closed source goes back to the list"
	);

	app.key(zgx::input::Key::Escape);
	app.frames(3);
	assert_eq!(
		last_call(&calls, "voice.go_live"),
		Some(json!(false)),
		"Escape closes the picker"
	);
	shows(
		&mut app,
		"the call view again",
		"grace",
		&[("voice.screens", closed_picker())],
	);
	assert!(app.find("go_live_close").is_none(), "the picker is closed");

	calls.borrow_mut().clear();
	app.click("call_screen");
	app.frames(2);
	shows(
		&mut app,
		"the picker again",
		"Mock Browser",
		&[("voice.screens", share_sources())],
	);
	app.click("go_live_close");
	app.frames(3);
	assert_eq!(last_call(&calls, "voice.go_live"), Some(json!(false)));
	shows(
		&mut app,
		"the call view after Close",
		"grace",
		&[("voice.screens", closed_picker())],
	);
	app.click("call_screen");
	app.frames(2);
	shows(
		&mut app,
		"the picker once more",
		"Mock Browser",
		&[("voice.screens", share_sources())],
	);
	app.click("go_live_cancel");
	app.frames(3);
	assert_eq!(last_call(&calls, "voice.go_live"), Some(json!(false)));
}

#[test]
fn going_live_shows_your_live_badge_and_the_screen_button_stops_it() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	shows(&mut app, "the call view", "grace", &call_values());
	let badges =
		|app: &mut zgx_script::testing::TestApp| app.texts().iter().filter(|text| *text == "LIVE").count();
	let before = badges(&mut app);
	let mut live = call_values();
	for (key, value) in &mut live {
		if *key == "voice.call" {
			value["tiles"][0]["live"] = json!(true);
		}
	}
	live.push((
		"voice.controls",
		json!({
			"muted": false, "deafened": false, "server_muted": false, "server_deafened": false,
			"camera": false, "live": true,
		}),
	));
	shows(&mut app, "your stream", "grace", &live);
	assert_eq!(badges(&mut app), before + 1, "your own tile shows LIVE");
	app.click("call_screen");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.stop_go_live"), Some(json!(true)));
	assert!(
		last_call(&calls, "voice.go_live").is_none(),
		"while live the screen button stops the stream instead of opening the picker"
	);
}
