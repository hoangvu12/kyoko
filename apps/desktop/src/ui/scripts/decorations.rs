use serde_json::json;

use super::{Calls, WIDE, member_window, recording_app, server_values, shows};

const MONITOR: &str = r#"code:<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor"><rect width="20" height="14" x="2" y="3" rx="2"/></svg>"#;

#[test]
fn a_member_row_shows_the_crown_and_plugin_decorations_only_when_published() {
	let calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let mut values = server_values(false);
	values.push(("members.heights", json!([40, 44, 44])));
	values.push(("members.window", member_window()));
	shows(&mut app, "a member list", "ada_lovelace", &values);
	assert!(app.find("member_crown_30").is_none());
	assert!(
		app.find("member_decoration_platform-indicators_monitor_30")
			.is_none()
	);

	let mut decorated = member_window();
	decorated["rows"][1]["crown"] = json!(true);
	decorated["rows"][1]["decorations"] = json!([
		{
			"key": "platform-indicators_monitor", "src": MONITOR,
			"color": "$kyoko.status_online", "tooltip": "Online on Desktop",
		},
		{ "key": "broken_icon", "src": "", "color": "$kyoko.text_4", "tooltip": "" },
	]);
	shows(
		&mut app,
		"a crowned member with a platform icon",
		"ada_lovelace",
		&[("members.window", decorated)],
	);
	assert!(app.find("member_crown_30").is_some(), "the owner's crown shows");
	assert!(
		app.find("member_decoration_platform-indicators_monitor_30")
			.is_some(),
		"the platform icon shows"
	);
	assert!(
		app.find("member_decoration_broken_icon_30").is_none(),
		"a mark without an icon is left out"
	);
}
