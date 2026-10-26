use serde_json::json;

use super::{Calls, WIDE, last_call, member_window, recording_app, server_values, shows};

pub(super) fn hidden_view() -> serde_json::Value {
	json!({
		"open": true, "id": "22", "name": "staff-only", "kind": "Text Channel", "icon": "hash",
		"topic": "talk about staff-only here", "activity": "Last message 3 days ago, October 23, 2026 9:41 AM",
		"created": "Created January 4, 2025", "nsfw": false, "slowmode": "Slowmode 10s",
		"allowed": [{ "key": "41", "name": "Admin", "member": false, "color": "#e74c3c" }],
		"denied": [
			{ "key": "10", "name": "@everyone", "member": false, "color": "" },
			{ "key": "30", "name": "ada", "member": true, "color": "" },
		],
	})
}

pub(super) fn closed_view() -> serde_json::Value {
	json!({
		"open": false, "id": "", "name": "", "kind": "", "icon": "hash", "topic": "", "activity": "",
		"created": "", "nsfw": false, "slowmode": "", "allowed": [], "denied": [],
	})
}

fn rows_with_hidden() -> serde_json::Value {
	let mut rows = super::sidebar_rows();
	if let Some(list) = rows.as_array_mut() {
		list.push(json!({
			"kind": "channel", "key": "h22", "id": "22", "type": "text", "icon": "hash", "name": "staff-only",
			"unread": false, "mentions": 0, "muted": false, "locked": true, "joined": false,
			"selectable": false, "hidden": true, "thread": false, "last_thread": false, "voice": [],
		}));
	}
	rows
}

#[test]
fn a_hidden_channel_opens_its_details_and_never_the_timeline() {
	let calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let mut values = server_values(false);
	values.push(("sidebar.rows", rows_with_hidden()));
	values.push(("composer.open", json!(true)));
	values.push(("composer.placeholder", json!("Message #general")));
	shows(&mut app, "a server with a hidden channel", "staff-only", &values);
	shows(&mut app, "the open channel's composer", "Message #general", &[]);
	app.click("channel_22");
	app.frames(2);
	assert_eq!(last_call(&calls, "sidebar.open_hidden"), Some(json!("22")));
	assert!(
		last_call(&calls, "sidebar.select_channel").is_none(),
		"a hidden channel never asks for its timeline"
	);
	shows(
		&mut app,
		"the hidden channel's details",
		"talk about staff-only here",
		&[("hidden.view", hidden_view())],
	);
	let texts = app.texts();
	for marker in [
		"ALLOWED TO VIEW",
		"NOT ALLOWED TO VIEW",
		"Admin",
		"@everyone",
		"Slowmode 10s",
		"Last message 3 days ago, October 23, 2026 9:41 AM",
		"Text Channel · You do not have access to this channel",
	] {
		assert!(
			texts.iter().any(|text| text == marker),
			"the details do not show \"{marker}\": {texts:?}"
		);
	}
	assert!(
		app.find("hidden_denied_30").is_some(),
		"a member overwrite is listed"
	);
	assert!(
		!texts.iter().any(|text| text == "Message #general"),
		"the composer is not shown for a hidden channel"
	);
	shows(
		&mut app,
		"the channel after the details close",
		"general",
		&[("hidden.view", closed_view())],
	);
	assert!(app.find("hidden_channel_view").is_none());
}

#[test]
fn moderator_notes_mark_a_member_only_when_published() {
	let calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let mut values = server_values(false);
	values.push(("members.heights", json!([40, 44, 44])));
	values.push(("members.window", member_window()));
	shows(&mut app, "a member list", "ada_lovelace", &values);
	assert!(app.find("member_timeout_30").is_none());
	let mut noted = member_window();
	noted["rows"][1]["timeout"] = json!("Timed out until October 28, 2026 9:41 AM");
	noted["rows"][1]["notes"] = json!("Pending membership screening");
	shows(
		&mut app,
		"a member with moderator notes",
		"ada_lovelace",
		&[("members.window", noted)],
	);
	assert!(app.find("member_timeout_30").is_some(), "the timeout shows");
	assert!(app.find("member_flags_30").is_some(), "the screening note shows");
}
