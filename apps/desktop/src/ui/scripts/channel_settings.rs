use serde_json::json;
use zgx_script::Value;

use super::{Calls, TALL, WIDE, last_call, recording_app, server_values, shows};

fn channel_info() -> serde_json::Value {
	json!({
		"id": "21", "name": "Lounge", "label": "Voice", "icon": "volume",
		"pages": [
			{ "key": "overview", "title": "Overview", "icon": "settings" },
			{ "key": "permissions", "title": "Permissions", "icon": "shield" },
			{ "key": "invites", "title": "Invites", "icon": "mail" },
		],
	})
}

fn voice_overview() -> serde_json::Value {
	json!({
		"id": "21", "name": "Lounge", "topic": "", "slowmode": 1, "visibility": "default", "nsfw": false,
		"announcement": false, "archive": 3, "bitrate": 64, "bitrate_min": 8, "bitrate_max": 128,
		"user_limit": 0, "user_limit_max": 99, "video_quality": 1, "region": 1,
		"regions": ["Automatic", "US East", "Rotterdam"], "category": 1, "categories": ["Voice Channels"],
		"audio": true, "has_topic": false, "archives": false, "announces": false,
	})
}

fn text_overview() -> serde_json::Value {
	json!({
		"id": "20", "name": "general", "topic": "say hi", "slowmode": 1, "visibility": "default", "nsfw": false,
		"announcement": false, "archive": 3, "bitrate": 64, "bitrate_min": 8, "bitrate_max": 128,
		"user_limit": 0, "user_limit_max": 99, "video_quality": 1, "region": 1,
		"regions": ["Automatic"], "category": 1, "categories": ["Text"],
		"audio": false, "has_topic": true, "archives": true, "announces": true,
	})
}

fn overwrites() -> serde_json::Value {
	json!({
		"rows": [
			{ "key": "10|r", "id": "10", "name": "@everyone", "color": "", "member": false, "everyone": true, "editable": true },
			{ "key": "41|r", "id": "41", "name": "Mods", "color": "#2ecc71", "member": false, "everyone": false, "editable": true },
		],
		"groups": [
			{ "title": "General Channel Permissions", "rights": [
				{ "index": 1, "name": "View Channel", "hint": "Members can view this channel." },
				{ "index": 2, "name": "Manage Channel", "hint": "Edit or delete this channel." },
			] },
		],
		"editing": "41",
	})
}

fn overwrite() -> serde_json::Value {
	json!({
		"id": "41", "name": "Mods", "color": "#2ecc71", "member": false, "everyone": false, "editable": true,
		"choices": { "2": -1 }, "serial": "41|0|16",
	})
}

fn channel_values(page: &str, overview: serde_json::Value) -> Vec<(&'static str, serde_json::Value)> {
	let mut values = server_values(false);
	values.extend([
		("channel.info", channel_info()),
		(
			"channel.choices",
			json!({ "slowmode": ["Off", "5s", "10s"], "archive": ["1 Hour", "24 Hours", "3 Days", "1 Week"] }),
		),
		("channel.overview", overview),
		("channel.overwrites", overwrites()),
		("channel.overwrite", overwrite()),
		(
			"channel.candidates",
			json!([{ "id": "43", "name": "Muted", "color": "", "member": false }]),
		),
		(
			"channel.invites",
			json!({ "loaded": true, "rows": [{
				"id": "mockinvite1", "inviter": "ada", "color": "", "avatar": "", "channel": "",
				"uses": "3", "expires": "Never", "temporary": false,
			}] }),
		),
		("channel.page", json!(page)),
	]);
	values
}

fn manageable_rows() -> serde_json::Value {
	let mut rows = super::sidebar_rows();
	for row in rows.as_array_mut().into_iter().flatten() {
		if row["kind"] == "channel" {
			row["manageable"] = json!(true);
		}
	}
	rows
}

#[test]
fn channel_settings_save_changed_fields_edit_overwrites_and_close() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let mut values = server_values(false);
	values.push(("sidebar.rows", manageable_rows()));
	shows(&mut app, "a server", "general", &values);
	let row = app.bounds("channel_21");
	app.ui.dispatch(zgx::input::InputEvent::PointerMove {
		x: row.x + row.width / 2.,
		y: row.y + row.height / 2.,
	});
	app.frames(2);
	let gear = app.bounds("channel_gear_21");
	assert!(gear.width > 0., "the gear has room in the row: {gear:?}");
	app.click("channel_gear_21");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.edit_channel"),
		Some(json!("21")),
		"the voice row's gear opens Edit Channel"
	);

	shows(
		&mut app,
		"a voice channel's overview",
		"Content Visibility",
		&channel_values("overview", voice_overview()),
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, TALL);
	app.frames(20);
	for text in [
		"Bitrate",
		"Video Quality",
		"User Limit",
		"Region Override",
		"Delete Channel",
	] {
		assert!(
			app.texts().iter().any(|shown| shown == text),
			"the voice overview shows {text}"
		);
	}
	assert!(app.find("settings_save_bar").is_none(), "nothing changed yet");
	app.click("channel_visibility_2");
	app.click("channel_video_quality_2");
	app.frames(2);
	app.click("settings_save_changes");
	assert_eq!(
		last_call(&calls, "channel.save"),
		Some(json!({ "visibility": "spoiler", "video_quality": 2 })),
		"Save sends only the changed fields"
	);

	shows(
		&mut app,
		"a text channel's overview",
		"Hide After Inactivity",
		&[("channel.overview", text_overview())],
	);
	app.frames(2);
	for text in ["Channel Topic", "Age-Restricted Channel", "Announcement Channel"] {
		assert!(
			app.texts().iter().any(|shown| shown == text),
			"the text overview shows {text}"
		);
	}

	shows(
		&mut app,
		"the permissions page",
		"Manage Channel",
		&[("channel.page", json!("permissions"))],
	);
	app.frames(2);
	app.click("overwrite_1_2");
	app.frames(2);
	app.click("settings_save_changes");
	assert_eq!(
		last_call(&calls, "channel.overwrite_save"),
		Some(json!({ "choices": [{ "index": 1, "value": 1 }, { "index": 2, "value": -1 }] })),
		"Save sends every shown permission with its allow, inherit or deny"
	);
	app.click("overwrite_row_10");
	assert_eq!(
		last_call(&calls, "channel.overwrite_open"),
		Some(json!({ "id": "10", "member": false }))
	);
	app.click("overwrite_remove");
	assert!(last_call(&calls, "channel.overwrite_remove").is_some());

	shows(
		&mut app,
		"the invites page",
		"mockinvite1",
		&[("channel.page", json!("invites"))],
	);
	app.click("channel_settings_nav_delete");
	assert!(
		last_call(&calls, "channel.delete").is_some(),
		"Delete Channel asks the host"
	);
	app.click("channel_settings_nav_overview");
	assert_eq!(last_call(&calls, "channel.open"), Some(json!("overview")));
	app.key(zgx::input::Key::Escape);
	assert!(
		last_call(&calls, "channel.close").is_some(),
		"Escape closes channel settings"
	);
	app.host.shared("channel.page", Value::Nil).set(Value::from(""));
	app.frames(2);
}
