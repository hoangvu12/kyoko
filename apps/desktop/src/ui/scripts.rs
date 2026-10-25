use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::{themes_dir, ui_dir};
use crate::theme::Catalog;

const WIDE: (f32, f32) = (1280., 800.);

#[test]
fn every_script_compiles() {
	let root = ui_dir();
	let files = files_with(&root, "luau");
	assert!(!files.is_empty(), "no scripts under {}", root.display());
	let failures: Vec<String> = files
		.iter()
		.filter_map(|file| compile(&root, file).err())
		.collect();
	assert!(
		failures.is_empty(),
		"scripts that do not compile:\n{}",
		failures.join("\n")
	);
}

#[test]
fn the_interface_loads_and_builds_every_screen() {
	let mut app = TestApp::new(|runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, "dark");
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			host.action(name.clone(), |_| {});
			host.function(name, |_| Value::Nil);
		}
		host
	});
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"the terms notice",
		"Accept the notice below to use QR login.",
		&[],
	);
	shows(
		&mut app,
		"the token form",
		"QR login is coming soon.",
		&[("login.acknowledged", json!(true))],
	);
	shows(
		&mut app,
		"the direct messages",
		"DIRECT MESSAGES",
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("dms.rows", dm_rows()),
			("dms.requests", json!(2)),
		],
	);
	shows(
		&mut app,
		"a server with its channels and members",
		"Playing Chess",
		&[
			("sidebar.guild_id", json!("10")),
			("sidebar.guild_name", json!("Kyoko")),
			("sidebar.rows", sidebar_rows()),
			("timeline.channel_id", json!("20")),
			("timeline.channel_name", json!("general")),
			("composer.open", json!(true)),
			("members.heights", json!([44, 42, 42])),
			("members.window", member_window()),
		],
	);
	shows(
		&mut app,
		"the settings view",
		"Edit User Profile",
		&[
			("settings.values", setting_values(true)),
			("settings.account", settings_account()),
			(
				"settings.about",
				json!({ "data_dir": "C:\\kyoko", "version": "0.0.0" }),
			),
			("settings.page", json!("my-account")),
			("plugins.list", plugin_list()),
		],
	);
	for (page, marker) in SETTINGS_PAGES {
		shows(&mut app, page, marker, &[("settings.page", json!(page))]);
	}
	shows(
		&mut app,
		"the closed settings view",
		"Playing Chess",
		&[("settings.page", json!(""))],
	);
	shows(
		&mut app,
		"a plugin notice",
		"ClearURLs was stopped because it ran past its time limit.",
		&[(
			"plugins.notice",
			json!({ "plugin": "clear-urls", "text": "ClearURLs was stopped because it ran past its time limit." }),
		)],
	);
	shows(
		&mut app,
		"the user panel with a custom status",
		"brb coffee",
		&[(
			"user.panel",
			json!({
				"name": "ada", "subtitle": "brb coffee", "emoji": "\u{2615}", "emoji_image": "",
				"status": "dnd", "latency": "Latency: 42 ms",
			}),
		)],
	);
	shows(
		&mut app,
		"the search results",
		"i spent the weekend on the parser",
		&[
			(
				"search.panel",
				json!({ "open": true, "title": "2 Results", "notice": "", "busy": false, "more": true }),
			),
			("search.chips", json!(["from: ada", "has: link"])),
			("search.hits", search_hits()),
		],
	);
	shows(
		&mut app,
		"the closed search results",
		"Playing Chess",
		&[(
			"search.panel",
			json!({ "open": false, "title": "", "notice": "", "busy": false, "more": false }),
		)],
	);
	shows(
		&mut app,
		"a home page",
		"Coming in a later version.",
		&[("sidebar.guild_id", json!("")), ("home.page", json!("friends"))],
	);
}

const SETTINGS_PAGES: [(&str, &str); 16] = [
	("my-account", "Edit User Profile"),
	("profiles", "Your profile is edited in Discord's web app for now."),
	("privacy", "Warn before opening links"),
	("devices", "Devices are managed in Discord's web app."),
	("connections", "Connections are managed in Discord's web app."),
	("clips", "Clips are not supported in Kyoko."),
	(
		"friend-requests",
		"Who can send you friend requests is managed in Discord's web app.",
	),
	("appearance", "Cozy: modern, beautiful, and easy to read."),
	("accessibility", "Play them when hovered"),
	("chat", "Original image quality"),
	("notifications", "Enable Desktop Notifications"),
	("keybinds", "Show or Hide Kyoko"),
	("language", "English (US)"),
	("streamer-mode", "Hide sensitive details while you stream."),
	("advanced", "Open Kyoko on Login"),
	("plugins", "Strips tracking parameters from the links you send."),
];

#[test]
fn the_plugins_page_asks_before_a_plugin_is_enabled() {
	let approved: std::rc::Rc<std::cell::RefCell<Vec<String>>> = std::rc::Rc::default();
	let seen = approved.clone();
	let mut app = TestApp::new(move |runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, "dark");
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			host.action(name.clone(), |_| {});
			host.function(name, |_| Value::Nil);
		}
		host.action("plugins.approve", move |argument| {
			seen.borrow_mut().push(argument.to_json())
		});
		host
	});
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let mut list = plugin_list();
	list[0]["enabled"] = json!(false);
	shows(
		&mut app,
		"the plugins page",
		"Strips tracking parameters from the links you send.",
		&[
			("login.screen", json!("main")),
			("settings.values", setting_values(true)),
			("settings.page", json!("plugins")),
			("plugins.list", list),
		],
	);
	assert!(
		!app.texts().iter().any(|text| text == "Review Permissions"),
		"a plugin that was never enabled asks on its switch, not with a banner button"
	);
	app.click("plugin_switch_clear-urls");
	shows(&mut app, "the permission prompt", "Allow ClearURLs?", &[]);
	assert!(app.texts().iter().any(|text| text == "THIS PLUGIN ASKS FOR"));
	app.click("plugin_allow");
	app.frames(2);
	assert_eq!(
		approved.borrow().as_slice(),
		[json!({ "id": "clear-urls", "permissions": ["hook:send"] }).to_string()],
		"approving sends exactly the permissions the prompt showed"
	);
	app.click("plugin_settings_clear-urls");
	shows(&mut app, "the plugin's settings", "Remove referral tags", &[]);
	assert!(app.texts().iter().any(|text| text == "hook:send"));
}

const TYPING: &str = "ada is typing...";
const DOT_DIM: f32 = 0.35;

#[test]
fn the_typing_dots_animate_only_while_the_setting_allows() {
	let mut app = TestApp::new(|runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, "dark");
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			host.action(name.clone(), |_| {});
			host.function(name, |_| Value::Nil);
		}
		host
	});
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"someone typing",
		TYPING,
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("sidebar.guild_id", json!("10")),
			("sidebar.guild_name", json!("Kyoko")),
			("sidebar.rows", sidebar_rows()),
			("timeline.channel_id", json!("20")),
			("timeline.channel_name", json!("general")),
			("composer.open", json!(true)),
			("composer.typing", json!(TYPING)),
			("settings.values", setting_values(true)),
		],
	);
	let watched = watch_dots(&mut app, Duration::from_millis(1500));
	assert!(
		watched.iter().any(|dots| (dots[0] - DOT_DIM).abs() < 0.01),
		"with the animation on, the first dot dims as the others light: {watched:?}"
	);

	shows(
		&mut app,
		"someone typing",
		TYPING,
		&[("settings.values", setting_values(false))],
	);
	let watched = watch_dots(&mut app, Duration::from_millis(1500));
	assert!(
		watched.iter().all(|dots| dots == &[1., 1., 1.]),
		"with the animation off, the dots stay lit and still: {watched:?}"
	);
}

fn watch_dots(app: &mut TestApp, span: Duration) -> Vec<[f32; 3]> {
	let deadline = Instant::now() + span;
	let mut seen = Vec::new();
	while Instant::now() < deadline {
		app.frame();
		seen.push([
			app.opacity("typing_dot_0"),
			app.opacity("typing_dot_1"),
			app.opacity("typing_dot_2"),
		]);
		std::thread::sleep(Duration::from_millis(20));
	}
	seen.dedup();
	seen
}

fn setting_values(typing_animation: bool) -> serde_json::Value {
	let mut settings = store::Settings::default();
	settings.chat.typing_animation = typing_animation;
	serde_json::Value::Object(
		bridge::setting_values(&settings)
			.into_iter()
			.map(|(key, value)| (key.to_owned(), value))
			.collect(),
	)
}

fn search_hits() -> serde_json::Value {
	json!([
		{
			"key": "s50", "id": "50", "author": "ada", "avatar": "", "time": "Today at 9:41 AM",
			"place": "#general", "text": "i spent the weekend on the parser",
		},
		{
			"key": "s51", "id": "51", "author": "grace", "avatar": "", "time": "Yesterday at 6:02 PM",
			"place": "", "text": "the docs are missing a section",
		},
	])
}

fn plugin_list() -> serde_json::Value {
	json!([{
		"id": "clear-urls", "name": "ClearURLs", "version": "1.0.0",
		"description": "Strips tracking parameters from the links you send.", "authors": "Kyoko",
		"origin": "bundled", "origin_label": "Built in", "status": "approval", "status_label": "Needs approval",
		"enabled": true, "needs_approval": true, "notice": "", "error": "",
		"settings": [{
			"key": "referrals", "type": "boolean", "label": "Remove referral tags", "description": "",
			"value": true, "min": 0, "max": 0, "step": 0, "multiline": false, "options": [],
		}],
		"permissions": [{
			"name": "hook:send", "text": "Read and rewrite the messages you send and edit, before they leave.",
			"sensitive": true, "reserved": false,
		}],
	}])
}

fn settings_account() -> serde_json::Value {
	json!({
		"name": "Ada", "username": "ada", "email": "a•••@example.invalid", "phone": "",
		"accent": "", "status": "online", "avatar": "",
	})
}

fn shows(app: &mut TestApp, screen: &str, marker: &str, values: &[(&str, serde_json::Value)]) {
	for (key, value) in values {
		let value = Value::from_json(&value.to_string()).unwrap_or(Value::Nil);
		app.host.shared(key, Value::Nil).set(value);
	}
	app.frames(2);
	if let Some(error) = app.error() {
		panic!("the interface failed while building {screen}:\n{error}");
	}
	let texts = app.texts();
	assert!(
		texts.iter().any(|text| text == marker),
		"{screen} does not show \"{marker}\"; it shows {texts:?}"
	);
}

fn compile(root: &Path, file: &Path) -> Result<(), String> {
	let name = file.strip_prefix(root).unwrap_or(file).display().to_string();
	let source = fs::read_to_string(file).map_err(|error| format!("{name}: {error}"))?;
	mlua::Lua::new()
		.load(source)
		.set_name(format!("@{name}"))
		.into_function()
		.map(drop)
		.map_err(|error| error.to_string())
}

fn files_with(dir: &Path, extension: &str) -> Vec<PathBuf> {
	let mut found = Vec::new();
	let Ok(entries) = fs::read_dir(dir) else {
		return found;
	};
	for path in entries.flatten().map(|entry| entry.path()) {
		if path.is_dir() {
			found.extend(files_with(&path, extension));
		} else if path.extension().is_some_and(|found| found == extension) {
			found.push(path);
		}
	}
	found.sort();
	found
}

fn host_names() -> BTreeSet<String> {
	let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
	files_with(&sources, "rs")
		.iter()
		.filter_map(|file| fs::read_to_string(file).ok())
		.flat_map(|source| {
			source
				.split('"')
				.filter(|text| is_host_name(text))
				.map(str::to_owned)
				.collect::<Vec<_>>()
		})
		.collect()
}

fn is_host_name(text: &str) -> bool {
	let part =
		|part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'_');
	text.split_once('.')
		.is_some_and(|(area, name)| part(area) && part(name))
}

fn rail_items() -> serde_json::Value {
	let guild = |id: &str, name: &str, unread: bool, mentions: u32| {
		json!({
			"kind": "guild", "key": format!("g{id}"), "id": id, "name": name,
			"acronym": &name[..1], "letter": &name[..1], "icon": "", "unread": unread, "mentions": mentions,
		})
	};
	json!([
		guild("10", "Kyoko", true, 3),
		{
			"kind": "folder", "key": "f1", "id": "1", "name": "Friends", "color": "#5865f2",
			"tint": "#5865f266", "unread": false, "mentions": 0,
			"guilds": [guild("11", "Games", false, 0), guild("12", "Music", true, 0)],
		},
	])
}

fn dm_rows() -> serde_json::Value {
	json!([
		{
			"key": "d30", "id": "30", "name": "ada", "avatar": "", "status": "online",
			"group": false, "members": 2, "unread": true, "mentions": 1,
		},
		{
			"key": "d31", "id": "31", "name": "ada, grace", "avatar": "", "status": "offline",
			"group": true, "members": 3, "unread": false, "mentions": 0,
		},
	])
}

fn sidebar_rows() -> serde_json::Value {
	let channel = |id: &str, kind: &str, icon: &str, name: &str, voice: serde_json::Value| {
		json!({
			"kind": "channel", "key": format!("h{id}"), "id": id, "type": kind, "icon": icon, "name": name,
			"unread": true, "mentions": 1, "muted": false, "locked": false, "joined": false,
			"selectable": true, "thread": false, "last_thread": false, "voice": voice,
		})
	};
	json!([
		{ "kind": "category", "key": "c40", "id": "40", "name": "Text", "collapsed": false, "muted": false },
		channel("20", "text", "hash", "general", json!([])),
		channel(
			"21",
			"voice",
			"volume",
			"Lounge",
			json!([{
				"key": "v21-30", "id": "30", "name": "ada", "avatar": "", "muted": true,
				"deafened": false, "video": "camera", "live": true,
			}]),
		),
	])
}

fn member_window() -> serde_json::Value {
	json!({
		"first": 0,
		"rows": [
			{ "kind": "group", "key": "online", "title": "ONLINE — 1" },
			{
				"kind": "member", "key": "m30", "id": "30", "name": "ada", "color": "#f47b67",
				"status": "online", "activity": "Playing Chess", "bot": false, "badge": "", "avatar": "",
			},
			{ "kind": "placeholder", "key": "p2", "index": 2 },
		],
	})
}
