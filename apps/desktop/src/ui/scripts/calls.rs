use serde_json::json;

use super::{Calls, WIDE, dm_rows, last_call, rail_items, recording_app, shows};

const DM: &str = "30";
const GROUP: &str = "31";

pub(super) fn ringing() -> Vec<(&'static str, serde_json::Value)> {
	vec![(
		"voice.incoming",
		json!({ "open": true, "channel_id": DM, "name": "ada", "avatar": "", "group": false }),
	)]
}

fn quiet_incoming() -> serde_json::Value {
	json!({ "open": false, "channel_id": "", "name": "", "avatar": "", "group": false })
}

fn quiet_area() -> serde_json::Value {
	json!({ "shown": false, "channel_id": "", "joined": false, "name": "", "people": [] })
}

pub(super) fn quiet_values() -> Vec<(&'static str, serde_json::Value)> {
	vec![
		("voice.incoming", quiet_incoming()),
		("voice.dm_call", quiet_area()),
	]
}

fn person(id: &str, name: &str, ringing: bool) -> serde_json::Value {
	json!({ "id": id, "name": name, "avatar": "", "ringing": ringing })
}

pub(super) fn lobby_values() -> Vec<(&'static str, serde_json::Value)> {
	vec![
		("voice.incoming", quiet_incoming()),
		(
			"voice.dm_call",
			json!({
				"shown": true, "channel_id": GROUP, "joined": false, "name": "ada, grace",
				"people": [person("40", "ada", false), person("41", "grace", true)],
			}),
		),
	]
}

fn dm_open(channel_id: &str) -> Vec<(&'static str, serde_json::Value)> {
	vec![
		("login.screen", json!("main")),
		("rail.items", rail_items()),
		("dms.rows", dm_rows()),
		("sidebar.guild_id", json!("")),
		("timeline.channel_id", json!(channel_id)),
		("timeline.channel_name", json!("ada")),
		("composer.open", json!(true)),
	]
}

fn joined_values() -> Vec<(&'static str, serde_json::Value)> {
	let tile = |id: &str, name: &str, me: bool, ringing: bool| {
		json!({
			"key": format!("t{id}"), "id": id, "name": name, "avatar": "", "me": me, "muted": false,
			"deafened": false, "video": "", "live": false, "volume": 100, "local_muted": false,
			"ringing": ringing, "stream": false, "stream_key": "", "watching": false,
		})
	};
	vec![
		(
			"voice.call",
			json!({
				"open": false, "channel_id": DM, "name": "ada", "connected": true,
				"tiles": [tile("100", "you", true, false), tile("40", "ada", false, true)],
			}),
		),
		(
			"voice.dm_call",
			json!({
				"shown": true, "channel_id": DM, "joined": true, "name": "ada",
				"people": [person("100", "you", false), person("40", "ada", true)],
			}),
		),
	]
}

#[test]
fn the_incoming_call_answers_with_and_without_video_and_declines() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "the direct messages", "Direct Messages", &dm_open(DM));
	shows(&mut app, "the incoming call", "Incoming Call…", &ringing());
	assert!(app.find("incoming_call").is_some(), "the popup shows");
	app.click("incoming_call_accept");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.accept_call"),
		Some(json!({ "id": DM, "video": false }))
	);
	app.click("incoming_call_video");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.accept_call"),
		Some(json!({ "id": DM, "video": true }))
	);
	app.click("incoming_call_decline");
	app.frames(2);
	assert_eq!(last_call(&calls, "voice.decline_call"), Some(json!({ "id": DM })));
	shows(&mut app, "no call", "Direct Messages", &quiet_values());
	assert!(
		app.find("incoming_call").is_none(),
		"the popup closes with the ring"
	);
}

#[test]
fn a_dm_starts_calls_from_its_header_and_shows_the_call_above_the_chat() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a DM", "Direct Messages", &dm_open(GROUP));
	assert!(app.find("dm_call").is_none(), "no call area without a call");
	app.click("top_bar_call");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.start_call"),
		Some(json!({ "id": GROUP, "video": false }))
	);
	app.click("top_bar_video_call");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.start_call"),
		Some(json!({ "id": GROUP, "video": true }))
	);
	shows(&mut app, "a DM call you are not in", "Join Call", &lobby_values());
	let area = app.bounds("dm_call");
	let timeline = app.bounds("chat_timeline");
	assert!(
		area.height > 0. && timeline.y >= area.y + area.height,
		"the chat stays below the call: {area:?} {timeline:?}"
	);
	assert!(app.find("dm_call_person_41").is_some(), "a ringing person shows");
	app.click("dm_call_join_video");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "voice.start_call"),
		Some(json!({ "id": GROUP, "video": true }))
	);
	let mut values = dm_open(DM);
	values.extend(joined_values());
	shows(&mut app, "a DM call you are in", "you", &values);
	assert!(app.find("dm_call_view").is_some(), "the call view sits in the DM");
	assert!(
		app.find("call_view").is_none(),
		"the full page call view stays closed"
	);
	for tile in ["call_tile_100", "call_tile_40"] {
		let bounds = app.bounds(tile);
		assert!(bounds.width > 0. && bounds.height > 0., "{tile} is laid out");
	}
	let before = app.bounds("dm_call").height;
	let handle = app.bounds("dm_call_resize");
	let (x, y) = (handle.x + handle.width / 2., handle.y + handle.height / 2.);
	let button = zgx::input::PointerButton::Primary;
	app.ui.dispatch(zgx::input::InputEvent::PointerMove { x, y });
	app.ui
		.dispatch(zgx::input::InputEvent::PointerDown { x, y, button });
	app.ui
		.dispatch(zgx::input::InputEvent::PointerMove { x, y: y + 60. });
	app.ui.dispatch(zgx::input::InputEvent::PointerUp {
		x,
		y: y + 60.,
		button,
	});
	app.frames(2);
	let after = app.bounds("dm_call").height;
	assert!(
		(after - before - 60.).abs() < 1.,
		"dragging the edge resizes the call: {before} -> {after}"
	);
	app.click("call_disconnect");
	app.frames(2);
	assert!(last_call(&calls, "voice.leave").is_some());
}
