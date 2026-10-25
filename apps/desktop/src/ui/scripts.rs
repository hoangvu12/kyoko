use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
	shows(&mut app, "the terms notice", &[]);
	shows(&mut app, "the token form", &[("login.acknowledged", json!(true))]);
	shows(
		&mut app,
		"the direct messages",
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
		"a home page",
		&[("sidebar.guild_id", json!("")), ("home.page", json!("friends"))],
	);
}

fn shows(app: &mut TestApp, screen: &str, values: &[(&str, serde_json::Value)]) {
	for (key, value) in values {
		let value = Value::from_json(&value.to_string()).unwrap_or(Value::Nil);
		app.host.shared(key, Value::Nil).set(value);
	}
	app.frames(2);
	if let Some(error) = app.error() {
		panic!("the interface failed while building {screen}:\n{error}");
	}
	assert!(!app.texts().is_empty(), "{screen} shows no text");
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
