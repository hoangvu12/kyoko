use serde_json::json;
use zgx::input::InputEvent;
use zgx_script::Value;
use zgx_script::testing::TestApp;

use super::{Calls, WIDE, last_call, recording_app, setting_values, shows};

fn plugin(id: &str, name: &str, enabled: bool, approval: bool, settings: usize) -> serde_json::Value {
	let (status, label) = match (enabled, approval) {
		(true, true) => ("approval", "Needs approval"),
		(true, false) => ("running", "Enabled"),
		_ => ("stopped", "Disabled"),
	};
	let settings: Vec<serde_json::Value> = (1..=settings)
		.map(|index| {
			json!({
				"key": format!("flag_{index}"), "type": "boolean", "label": format!("{name} switch {index}"),
				"description": "A setting with a description under it.", "value": true, "min": 0, "max": 0,
				"step": 0, "multiline": false, "options": [], "fields": [], "item": "", "rows": 0,
			})
		})
		.collect();
	json!({
		"id": id, "name": name, "version": "1.0.0",
		"description": format!("{name} does one small thing, explained in a sentence long enough to wrap."),
		"authors": "Kyoko", "origin": "bundled", "origin_label": "Built in",
		"status": status, "status_label": label, "enabled": enabled, "needs_approval": approval,
		"notice": "", "error": "", "settings": settings,
		"permissions": [
			{ "name": "ui:panel.buttons", "text": "Add to the interface at panel.buttons.", "sensitive": false, "reserved": false },
			{ "name": "action:activity", "text": "Change whether your activity is shared when you click its button.", "sensitive": true, "reserved": false },
		],
	})
}

fn seven(changed: Option<(&str, bool, bool)>) -> serde_json::Value {
	let rows = [
		("clear-urls", "ClearURLs", true, false, 1),
		("dont-round", "DontRoundMyTimestamps", false, false, 0),
		("cloner", "ExpressionCloner", false, false, 0),
		("favourite", "FavoriteEmojiFirst", true, false, 1),
		("game-activity", "GameActivityToggle", false, true, 0),
		("message-logger", "MessageLogger", true, false, 16),
		("one-ping", "OnePingPerDM", false, false, 1),
	];
	serde_json::Value::Array(
		rows.iter()
			.map(|&(id, name, enabled, approval, settings)| match changed {
				Some((wanted, on, asks)) if wanted == id => plugin(id, name, on, asks, settings),
				_ => plugin(id, name, enabled, approval, settings),
			})
			.collect(),
	)
}

fn publish(app: &mut TestApp, list: serde_json::Value) {
	let value = Value::from_json(&list.to_string()).unwrap_or(Value::Nil);
	app.host.shared("plugins.list", Value::Nil).set(value);
	app.frames(3);
}

fn scroll_page(app: &mut TestApp, by: f32) {
	let page = app.bounds("settings_page");
	app.ui.dispatch(InputEvent::Scroll {
		x: page.x + page.width / 2.,
		y: page.y + page.height / 2.,
		delta_x: 0.,
		delta_y: by,
	});
	app.frames(3);
}

fn top_of(app: &mut TestApp, id: &str) -> f32 {
	app.ui.prepare_frame();
	app.bounds(id).y
}

fn open_page(calls: &Calls) -> TestApp {
	let mut app = recording_app(calls, "");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"the plugins page",
		"MessageLogger",
		&[
			("login.screen", json!("main")),
			("settings.values", setting_values(true)),
			("settings.page", json!("plugins")),
			("plugins.list", seven(None)),
		],
	);
	app.frames(20);
	app
}

fn outside_the_prompt(app: &mut TestApp) -> (f32, f32) {
	let panel = app.bounds("plugin_prompt");
	(panel.x + panel.width / 2., panel.y + panel.height + 20.)
}

fn press(app: &mut TestApp, (x, y): (f32, f32)) {
	use zgx::input::PointerButton;
	app.ui.dispatch(InputEvent::PointerDown {
		x,
		y,
		button: PointerButton::Primary,
	});
	app.ui.dispatch(InputEvent::PointerUp {
		x,
		y,
		button: PointerButton::Primary,
	});
	app.frames(3);
}

fn open_prompt(app: &mut TestApp) {
	app.click("plugin_switch_game-activity");
	let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
	while !allow_in_place(app) {
		assert!(
			std::time::Instant::now() < deadline,
			"the prompt finishes sliding in"
		);
		std::thread::sleep(std::time::Duration::from_millis(10));
		app.frames(1);
	}
}

fn allow_in_place(app: &mut TestApp) -> bool {
	let Some(node) = app.find("plugin_allow") else {
		return false;
	};
	let Some(panel) = app.find("plugin_prompt") else {
		return false;
	};
	let scene = app.ui.scene.borrow();
	let window = scene.bounds(app.ui.root());
	let panel = scene.bounds(panel);
	scene.visible_bounds(node).is_some() && ((panel.y + panel.height / 2.) - window.height / 2.).abs() < 1.
}

fn prompt_shown(app: &mut TestApp) -> bool {
	app.ui.prepare_frame();
	let Some(node) = app.find("plugin_prompt") else {
		return false;
	};
	let shown = app
		.ui
		.scene
		.borrow()
		.visible_bounds(node)
		.is_some_and(|bounds| bounds.width > 0.);
	shown && app.texts().iter().any(|text| text == "Allow GameActivityToggle?")
}

#[test]
fn the_plugins_page_keeps_its_scroll_and_asks_on_a_scrim() {
	let calls = Calls::default();
	let mut app = open_page(&calls);
	let unscrolled = top_of(&mut app, "plugin_switch_message-logger");
	scroll_page(&mut app, 300.);
	let scrolled = top_of(&mut app, "plugin_switch_message-logger");
	assert!(
		scrolled < unscrolled - 100.,
		"the page scrolls: {unscrolled} then {scrolled}"
	);

	app.click("plugin_switch_clear-urls");
	assert_eq!(last_call(&calls, "plugins.disable"), Some(json!("clear-urls")));
	publish(&mut app, seven(Some(("clear-urls", false, false))));
	assert_eq!(
		top_of(&mut app, "plugin_switch_message-logger"),
		scrolled,
		"turning a plugin off"
	);

	open_prompt(&mut app);
	assert!(prompt_shown(&mut app), "a plugin never approved asks first");
	let window = app.ui.scene.borrow().bounds(app.ui.root());
	let scrim = app.bounds("plugin_prompt_scrim");
	assert!(
		scrim.width >= window.width - 1. && scrim.height >= window.height - 1.,
		"the scrim covers the window: {scrim:?} in {window:?}"
	);
	let panel = app.bounds("plugin_prompt");
	assert!(
		(panel.x + panel.width / 2. - window.width / 2.).abs() < 2.,
		"the prompt is centred: {panel:?}"
	);
	let outside = outside_the_prompt(&mut app);
	press(&mut app, outside);
	assert!(!prompt_shown(&mut app), "a press on the scrim closes the prompt");

	open_prompt(&mut app);
	app.click("plugin_cancel");
	assert!(!prompt_shown(&mut app), "Cancel closes the prompt");
	assert_eq!(
		top_of(&mut app, "plugin_switch_message-logger"),
		scrolled,
		"cancelling"
	);

	open_prompt(&mut app);
	app.click("plugin_allow");
	assert_eq!(
		last_call(&calls, "plugins.approve"),
		Some(json!({ "id": "game-activity", "permissions": ["ui:panel.buttons", "action:activity"] }))
	);
	publish(&mut app, seven(Some(("game-activity", true, false))));
	assert_eq!(
		top_of(&mut app, "plugin_switch_message-logger"),
		scrolled,
		"approving a plugin"
	);

	for plugin in ["message-logger", "dont-round"] {
		let page = app.bounds("settings_page").y;
		app.click(&format!("plugin_settings_{plugin}"));
		app.frames(3);
		let back = top_of(&mut app, "plugin_back");
		assert!(
			back - page < 80.,
			"{plugin}'s settings open at their top: {back} in a page at {page}"
		);
		app.click("plugin_back");
		app.frames(3);
		assert_eq!(
			top_of(&mut app, "plugin_switch_message-logger"),
			scrolled,
			"closing {plugin}'s settings returns to where the list was"
		);
	}
}
