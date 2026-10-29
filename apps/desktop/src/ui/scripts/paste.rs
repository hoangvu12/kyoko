use serde_json::json;
use zgx::input::{Key, Modifiers};
use zgx::platform::Platform as _;
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::{Calls, WIDE, forum_tags, forum_view, host_names, server_values, shows, thread_panel};
use crate::theme::Catalog;
use crate::ui::{themes_dir, ui_dir};

const COPIED_PATH: &str = "C:\\Users\\me\\Pictures\\cat.png";
const PASTERS: [&str; 3] = ["composer.key", "thread_composer.key", "forum.paste"];

fn pasting_app(calls: &Calls) -> TestApp {
	let calls = calls.clone();
	TestApp::with_desktop(move |runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, crate::theme::DEFAULT);
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			host.action(name.clone(), |_| {});
			host.function(name, |_| Value::Nil);
		}
		for name in PASTERS {
			let seen = calls.clone();
			host.function(name, move |argument| {
				seen.borrow_mut().push((name.to_owned(), argument.to_json()));
				let key = crate::ui::field(&argument, "key").and_then(crate::ui::text);
				Value::Bool(name == "forum.paste" || key.is_some_and(|key| key.eq_ignore_ascii_case("v")))
			});
		}
		host
	})
}

fn paste_into(app: &mut TestApp, field: &str) {
	app.click(field);
	app.key_with(
		Key::Character("v".into()),
		Modifiers {
			control: true,
			..Modifiers::default()
		},
	);
	app.frames(2);
}

fn pasted_with(calls: &Calls, name: &str) -> Vec<serde_json::Value> {
	calls
		.borrow()
		.iter()
		.filter(|(called, _)| called == name)
		.filter_map(|(_, argument)| serde_json::from_str(argument).ok())
		.filter(|argument: &serde_json::Value| argument["cmd"] == json!(true) || argument.is_null())
		.collect()
}

#[test]
fn ctrl_v_in_the_composers_and_the_post_body_asks_the_host_and_never_pastes_a_path() {
	let calls = Calls::default();
	let mut app = pasting_app(&calls);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	if let Some(desktop) = &app.desktop {
		desktop.set_clipboard_text(COPIED_PATH.to_owned());
	}
	let mut values = server_values(false);
	values.push(("composer.open", json!(true)));
	values.push(("composer.placeholder", json!("Message #general")));
	values.push(("threads.panel", thread_panel(true)));
	values.push(("thread_composer.open", json!(true)));
	values.push(("thread_composer.placeholder", json!("Message #bug triage")));
	shows(
		&mut app,
		"a channel with a thread beside it",
		"Message #bug triage",
		&values,
	);
	paste_into(&mut app, "composer_field");
	paste_into(&mut app, "thread_composer_field");
	let composer = pasted_with(&calls, "composer.key");
	assert_eq!(composer.len(), 1, "{composer:?}");
	assert_eq!(composer[0]["key"], json!("v"));
	assert_eq!(pasted_with(&calls, "thread_composer.key").len(), 1);
	shows(
		&mut app,
		"a new forum post",
		"Add Tags",
		&[
			("threads.panel", thread_panel(false)),
			("timeline.channel_icon", json!("forum")),
			("forum.view", forum_view(true, "Recently Active", false, true)),
			("forum.tags", forum_tags()),
			("forum.files", json!([])),
		],
	);
	paste_into(&mut app, "forum_post_body");
	assert_eq!(pasted_with(&calls, "forum.paste").len(), 1);
	let texts = app.texts();
	assert!(
		!texts.iter().any(|text| text.contains(COPIED_PATH)),
		"a consumed paste left the path in a field: {texts:?}"
	);
}
