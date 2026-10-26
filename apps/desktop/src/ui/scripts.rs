use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::{themes_dir, ui_dir};
use crate::theme::Catalog;

mod plugins;

pub(super) const WIDE: (f32, f32) = (1280., 800.);
const TALL: f32 = 1400.;

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
	let probe = plugins::Lab::probe("load-check", &plugins::ALL_SLOTS);
	probe.approve_all();
	let (mut app, _probe) = plugins::app_with(probe, |lab, host| lab.mount(host, |_| {}));
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
		"a member's username beside the name",
		"ada_lovelace",
		&[("members.window", member_window())],
	);
	for marker in plugins::PROBE_MARKERS {
		shows(&mut app, "a plugin in every placed slot", marker, &[]);
	}
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
		"the server settings",
		"A mock guild",
		&[
			("server.guild", server_guild()),
			("server.rights", server_rights()),
			("server.audit_filters", json!(["All Actions", "Member Kick"])),
			(
				"server.overview",
				json!({ "name": "Kyoko", "description": "A mock guild", "icon": "" }),
			),
			("server.page", json!("overview")),
		],
	);
	for (page, marker, key, value) in server_pages() {
		shows(
			&mut app,
			page,
			marker,
			&[(key, value), ("server.page", json!(page))],
		);
	}
	shows(
		&mut app,
		"a role being edited",
		"Display role members separately",
		&[
			("server.page", json!("roles")),
			(
				"server.role",
				json!({
					"id": "41", "name": "Mods", "title": "Mods", "color": "#2ecc71", "hoist": true,
					"mentionable": false, "everyone": false, "deletable": true, "granted": [1, 4],
				}),
			),
		],
	);
	shows(
		&mut app,
		"the closed server settings",
		"Playing Chess",
		&[("server.page", json!("")), ("server.role", json!(null))],
	);
	shows(
		&mut app,
		"the reconnecting banner",
		"Reconnecting in 4s...",
		&[(
			"session.connection",
			json!({ "shown": true, "state": "reconnecting", "text": "Reconnecting in 4s...", "tone": "warning", "retry": true }),
		)],
	);
	shows(
		&mut app,
		"the offline banner",
		"You're offline. Retrying in 30s...",
		&[(
			"session.connection",
			json!({ "shown": true, "state": "offline", "text": "You're offline. Retrying in 30s...", "tone": "danger", "retry": true }),
		)],
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
				"emoji_svg": crate::emoji::luau_code("\u{2615}"),
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
		"a thread beside its channel",
		"Join bug triage to take part in the conversation.",
		&[
			("threads.panel", thread_panel(true)),
			("thread_composer.open", json!(true)),
			("thread_composer.placeholder", json!("Message #bug triage")),
		],
	);
	shows(
		&mut app,
		"a forum with its posts",
		"crash on startup",
		&[
			("threads.panel", thread_panel(false)),
			("timeline.channel_icon", json!("forum")),
			(
				"forum.view",
				json!({ "open": true, "id": "30", "sort": "Recently Active", "gallery": false }),
			),
			("forum.tags", forum_tags()),
			("forum.cards", forum_cards()),
		],
	);
	shows(
		&mut app,
		"a forum in gallery view",
		"how do i theme the sidebar",
		&[(
			"forum.view",
			json!({ "open": true, "id": "30", "sort": "Date Posted", "gallery": true }),
		)],
	);
	shows(
		&mut app,
		"the channel after the forum closes",
		"Playing Chess",
		&[(
			"forum.view",
			json!({ "open": false, "id": "", "sort": "Recently Active", "gallery": false }),
		)],
	);
	shows(
		&mut app,
		"a home page",
		"Coming in a later version.",
		&[("sidebar.guild_id", json!("")), ("home.page", json!("nitro"))],
	);
	shows(
		&mut app,
		"the friends list",
		"ONLINE — 2",
		&[
			("home.page", json!("friends")),
			("friends.rows", friend_rows()),
			("friends.pending", json!(1)),
			("friends.active", json!([friend_rows()[2]])),
		],
	);
	shows(&mut app, "friends active now", "Playing Hades II", &[]);
	shows(
		&mut app,
		"no pending friends",
		"There are no pending friend requests.",
		&[
			("friends.tab", json!("pending")),
			("friends.rows", json!([])),
			("friends.empty", json!("There are no pending friend requests.")),
			("friends.active", json!([])),
		],
	);
	shows(&mut app, "a quiet active now", "It's quiet for now...", &[]);
	shows(
		&mut app,
		"the add friend form",
		"Success! Your friend request to grace was sent.",
		&[
			("friends.tab", json!("add")),
			("friends.empty", json!("")),
			(
				"friends.add",
				json!({ "phase": "sent", "text": "Success! Your friend request to grace was sent." }),
			),
		],
	);
}

fn server_guild() -> serde_json::Value {
	let page = |key: &str, title: &str, icon: &str| json!({ "key": key, "title": title, "icon": icon });
	json!({
		"id": "10",
		"name": "Kyoko",
		"groups": [
			{ "title": "Server", "pages": [page("overview", "Overview", "settings"), page("roles", "Roles", "shield")] },
			{ "title": "People", "pages": [page("members", "Members", "members"), page("bans", "Bans", "ban")] },
			{ "title": "Moderation", "pages": [page("audit-log", "Audit Log", "audit_log")] },
		],
	})
}

fn server_rights() -> serde_json::Value {
	json!([{
		"title": "General Server Permissions",
		"rights": [
			{ "index": 1, "name": "View Channels", "hint": "Members can view channels." },
			{ "index": 4, "name": "Kick Members", "hint": "Remove other members." },
		],
	}])
}

fn server_pages() -> [(&'static str, &'static str, &'static str, serde_json::Value); 7] {
	[
		(
			"roles",
			"Create Role",
			"server.roles",
			json!({
				"count": 2,
				"rows": [
					{ "id": "41", "name": "Mods", "color": "#2ecc71", "members": 7, "everyone": false, "movable": true },
					{ "id": "10", "name": "@everyone", "color": "", "members": 50, "everyone": true, "movable": false },
				],
			}),
		),
		(
			"members",
			"brave_ember",
			"server.members",
			json!({
				"total": 1,
				"actions": true,
				"rows": [{
					"id": "30", "name": "Brave Ember", "username": "brave_ember", "color": "#5865f2", "avatar": "",
					"roles": [{ "name": "Members", "color": "#5865f2", "tint": "#5865f22e" }],
				}],
			}),
		),
		(
			"bans",
			"Spam links in every channel",
			"server.bans",
			json!({
				"loaded": true,
				"has_more": true,
				"rows": [{ "id": "31", "name": "Raider", "reason": "Spam links in every channel", "avatar": "" }],
			}),
		),
		(
			"audit-log",
			"Clever Lynx kicked Brave Ember",
			"server.audit",
			json!({
				"loaded": true,
				"has_more": false,
				"filter": 0,
				"rows": [{
					"id": "32", "sentence": "Clever Lynx kicked Brave Ember", "when": "10/01/2026", "avatar": "",
					"changes": [{ "key": "nick", "old": "a", "new": "b" }], "reason": "spam",
				}],
			}),
		),
		(
			"emoji",
			":pepe_ok:",
			"server.emoji",
			json!({
				"still": 1, "animated": 0, "slots": 50,
				"rows": [{ "id": "33", "name": "pepe_ok", "image": "", "uploader": "Ada", "avatar": "" }],
				"upload": { "serial": 1, "name": "wave", "preview": "" },
			}),
		),
		(
			"stickers",
			"crab_wave",
			"server.stickers",
			json!({
				"count": 1, "slots": 5,
				"rows": [{ "id": "34", "name": "crab_wave", "tags": "crab", "image": "" }],
			}),
		),
		(
			"invites",
			"rustaceans",
			"server.invites",
			json!({
				"rows": [{
					"id": "rustaceans", "inviter": "Ada", "avatar": "", "channel": "#general", "uses": "12",
					"expires": "Never", "temporary": true,
				}],
			}),
		),
	]
}

const SETTINGS_PAGES: [(&str, &str); 17] = [
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
	(
		"themes",
		"Paste direct links to .css or theme.json files, one per line. Start a line with @light or @dark to use it only with light or dark colours.",
	),
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

#[test]
fn the_generated_plugin_settings_build_every_field_type() {
	let calls = Calls::default();
	let mut app = recording_app(&calls, "");
	app.ui.scene.borrow_mut().resize(WIDE.0, TALL);
	shows(
		&mut app,
		"the plugins page",
		"Every setting type",
		&[
			("login.screen", json!("main")),
			("settings.values", setting_values(true)),
			("settings.page", json!("plugins")),
			("plugins.list", every_field_plugin()),
		],
	);
	app.click("plugin_settings_fields");
	shows(&mut app, "a plugin with every field type", "Rule 1", &[]);
	let texts = app.texts();
	for marker in [
		"A switch",
		"A stepper",
		"A text",
		"Second",
		"Run it",
		"Rules",
		"Rule 2",
		"Find",
		"Luau pattern",
		"Add Rule",
	] {
		assert!(
			texts.iter().any(|text| text == marker),
			"the settings page does not show \"{marker}\": {texts:?}"
		);
	}
	app.click("plugin_list_rules_add");
	shows(&mut app, "a list after adding a row", "Rule 3", &[]);
	assert_eq!(
		last_call(&calls, "plugins.set"),
		Some(json!({
			"id": "fields",
			"key": "rules",
			"value": [
				{ "find": "teh", "pattern": false },
				{ "find": "(%d+)km", "pattern": true },
				{ "find": "", "pattern": false },
			],
		}))
	);
}

fn every_field_plugin() -> serde_json::Value {
	let setting = |key: &str, kind: &str, label: &str, value: serde_json::Value| {
		json!({
			"key": key, "type": kind, "label": label, "description": "", "value": value,
			"min": 0, "max": 10, "step": 1, "multiline": false, "options": [],
			"fields": [], "item": "", "rows": 0,
		})
	};
	let mut select = setting("choice", "select", "A choice", json!("b"));
	select["options"] = json!([{ "value": "a", "label": "First" }, { "value": "b", "label": "Second" }]);
	let mut list = setting(
		"rules",
		"list",
		"Rules",
		json!([{ "find": "teh", "pattern": false }, { "find": "(%d+)km", "pattern": true }]),
	);
	list["item"] = json!("Rule");
	list["rows"] = json!(5);
	list["fields"] = json!([
		setting("find", "string", "Find", json!("")),
		setting("pattern", "boolean", "Luau pattern", json!(false)),
	]);
	json!([{
		"id": "fields", "name": "Fields", "version": "1.0.0",
		"description": "Every setting type", "authors": "Kyoko",
		"origin": "development", "origin_label": "Development", "status": "running", "status_label": "Enabled",
		"enabled": true, "needs_approval": false, "notice": "", "error": "",
		"settings": [
			setting("flag", "boolean", "A switch", json!(true)),
			setting("count", "number", "A stepper", json!(3)),
			setting("words", "string", "A text", json!("hello")),
			select,
			setting("run", "action", "Run it", serde_json::Value::Null),
			list,
		],
		"permissions": [],
	}])
}

#[test]
fn the_themes_page_applies_toggles_and_saves_links() {
	let calls: Calls = std::rc::Rc::default();
	let mut app = recording_app(&calls, "default");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"the themes page",
		"Midnight Ocean",
		&[
			("login.screen", json!("main")),
			("settings.values", setting_values(true)),
			("settings.page", json!("themes")),
			("themes.list", theme_list()),
		],
	);
	let texts = app.texts();
	for shown in [
		"APPLIED",
		"v2.1.0 · by someone",
		"Deep blue with a teal accent.",
		"the server answered 404 Not Found",
		"v1.0 · by Kyoko · dark only",
	] {
		assert!(texts.iter().any(|text| text == shown), "{shown} is not shown");
	}
	app.click("theme_apply_light");
	app.click("theme_switch_ocean.css");
	app.type_text("theme_links", "https://a.example/one.css");
	app.click("theme_links_save");
	app.frames(2);
	assert_eq!(last_call(&calls, "themes.apply"), Some(json!("light")));
	assert_eq!(
		last_call(&calls, "themes.toggle"),
		Some(json!({ "id": "ocean.css", "on": true }))
	);
	assert_eq!(
		last_call(&calls, "themes.set_links"),
		Some(json!("https://a.example/one.css"))
	);
}

fn theme_entry(id: &str, source: &str, name: &str, enabled: bool) -> serde_json::Value {
	json!({
		"id": id, "source": source, "name": name, "author": "", "description": "", "version": "",
		"enabled": enabled, "error": "", "note": "", "scheme": "",
		"swatch": ["#1e1f22", "#2b2d31", "#313338", "#5865f2", "#dbdee1", "#00a8fc"],
	})
}

fn theme_list() -> serde_json::Value {
	let mut ocean = theme_entry("ocean.css", "local", "Midnight Ocean", false);
	ocean["author"] = json!("someone");
	ocean["version"] = json!("2.1.0");
	ocean["description"] = json!("Deep blue with a teal accent.");
	let mut broken = theme_entry("https://a.example/broken.css", "online", "broken", false);
	broken["error"] = json!("the server answered 404 Not Found");
	let mut green = theme_entry("@dark https://a.example/green.css", "online", "Green", true);
	green["author"] = json!("Kyoko");
	green["version"] = json!("1.0");
	green["scheme"] = json!("dark");
	json!([
		theme_entry("dark", "builtin", "Dark", true),
		theme_entry("light", "builtin", "Light", false),
		ocean,
		broken,
		green,
	])
}

type Calls = std::rc::Rc<std::cell::RefCell<Vec<(String, String)>>>;

fn recording_app(calls: &Calls, level: &'static str) -> TestApp {
	let calls = calls.clone();
	TestApp::new(move |runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, "dark");
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			let (seen, recorded) = (calls.clone(), name.clone());
			host.action(name.clone(), move |argument| {
				seen.borrow_mut().push((recorded.clone(), argument.to_json()));
			});
			host.function(name, |_| Value::Nil);
		}
		host.function("sidebar.channel_level", move |_| Value::from(level));
		host
	})
}

fn last_call(calls: &Calls, name: &str) -> Option<serde_json::Value> {
	calls
		.borrow()
		.iter()
		.rev()
		.find(|(called, _)| called == name)
		.and_then(|(_, argument)| serde_json::from_str(argument).ok())
}

fn server_values(muted: bool) -> Vec<(&'static str, serde_json::Value)> {
	vec![
		("login.screen", json!("main")),
		("rail.items", rail_items()),
		("sidebar.guild_id", json!("10")),
		("sidebar.guild_name", json!("Kyoko")),
		("sidebar.rows", sidebar_rows()),
		(
			"sidebar.header",
			json!({
				"unread": true, "muted": muted, "owner": false, "can_invite": true,
				"can_manage_channels": false, "can_open_settings": false,
			}),
		),
		("timeline.channel_id", json!("20")),
		("timeline.channel_name", json!("general")),
		("settings.values", setting_values(true)),
	]
}

#[test]
fn the_server_menu_reaches_the_community_actions() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	for (item, action) in [
		("menu_invite", "sidebar.invite"),
		("menu_notifications", "sidebar.notification_settings"),
		("menu_leave", "sidebar.leave_guild"),
	] {
		app.click("server_menu");
		app.frames(2);
		app.click(item);
		app.frames(2);
		assert!(last_call(&calls, action).is_some(), "{item} calls {action}");
	}
	app.click("server_menu");
	app.frames(2);
	app.click("menu_mute_guild");
	app.frames(2);
	app.click("mute_guild_60");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.mute_guild"),
		Some(json!({ "id": "", "minutes": 60 })),
		"the mute submenu sends the chosen window"
	);
	shows(&mut app, "a muted server", "general", &server_values(true));
	app.click("server_menu");
	app.frames(2);
	app.click("menu_unmute_guild");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.mute_guild"),
		Some(json!({ "unmute": true }))
	);
}

fn open_channel_menu(app: &mut TestApp, id: &str) {
	use zgx::input::{InputEvent, PointerButton};
	let bounds = app.bounds(id);
	let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
	app.ui.dispatch(InputEvent::PointerMove { x, y });
	app.ui.dispatch(InputEvent::PointerDown {
		x,
		y,
		button: PointerButton::Secondary,
	});
	app.ui.dispatch(InputEvent::PointerUp {
		x,
		y,
		button: PointerButton::Secondary,
	});
	app.frames(2);
}

#[test]
fn the_channel_menu_mutes_invites_and_sets_notifications() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "a server", "general", &server_values(false));
	open_channel_menu(&mut app, "channel_20");
	app.click("menu_invite_channel");
	app.frames(2);
	assert_eq!(last_call(&calls, "sidebar.invite"), Some(json!("20")));
	open_channel_menu(&mut app, "channel_20");
	app.click("menu_mute_channel");
	app.frames(2);
	app.click("mute_channel_15");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.mute_channel"),
		Some(json!({ "id": "20", "minutes": 15 }))
	);
	open_channel_menu(&mut app, "channel_20");
	app.click("menu_channel_levels");
	app.frames(2);
	app.click("level_nothing");
	app.frames(2);
	assert_eq!(
		last_call(&calls, "sidebar.channel_notifications"),
		Some(json!({ "id": "20", "level": "nothing" }))
	);
}

#[test]
fn the_server_settings_pages_call_the_host() {
	let calls: std::rc::Rc<std::cell::RefCell<Vec<(String, serde_json::Value)>>> = std::rc::Rc::default();
	let seen = calls.clone();
	let mut app = TestApp::new(move |runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, "dark");
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			host.action(name.clone(), |_| {});
			host.function(name, |_| Value::Nil);
		}
		for name in ["server.open", "server.unban"] {
			let seen = seen.clone();
			host.action(name, move |argument| {
				let value = serde_json::from_str(&argument.to_json()).unwrap_or_default();
				seen.borrow_mut().push((name.to_owned(), value));
			});
		}
		host
	});
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let [_, _, (page, marker, key, value), ..] = server_pages();
	shows(
		&mut app,
		page,
		marker,
		&[
			("login.screen", json!("main")),
			("server.guild", server_guild()),
			(key, value),
			("server.page", json!(page)),
		],
	);
	app.click("ban_revoke_31");
	app.click("server_nav_roles");
	app.frames(2);
	assert_eq!(
		calls.borrow().as_slice(),
		[
			("server.unban".to_owned(), json!({ "id": "31", "name": "Raider" })),
			("server.open".to_owned(), json!("roles")),
		]
	);
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

fn thread_panel(open: bool) -> serde_json::Value {
	json!({
		"open": open, "id": "40", "name": "bug triage", "members": 3, "archived": false,
		"joined": false, "can_manage": true,
	})
}

fn forum_tags() -> serde_json::Value {
	json!([
		{ "key": "1", "id": "1", "label": "\u{1f41b} bug", "active": false },
		{ "key": "2", "id": "2", "label": "\u{2705} solved", "active": true },
	])
}

fn forum_cards() -> serde_json::Value {
	json!([
		{
			"key": "41", "id": "41", "title": "crash on startup", "excerpt": "that screenshot is hilarious",
			"tags": [{ "key": "1", "label": "\u{1f41b} bug", "moderated": false }], "author": "ada",
			"avatar": "", "replies": "14 replies", "activity": "Today at 9:41 AM", "pinned": true,
			"archived": false,
		},
		{
			"key": "42", "id": "42", "title": "how do i theme the sidebar", "excerpt": "",
			"tags": [{ "key": "2", "label": "\u{2705} solved", "moderated": true }], "author": "grace",
			"avatar": "", "replies": "1 reply", "activity": "Yesterday at 6:02 PM", "pinned": false,
			"archived": true,
		},
	])
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

pub(super) fn shows(app: &mut TestApp, screen: &str, marker: &str, values: &[(&str, serde_json::Value)]) {
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

pub(super) fn host_names() -> BTreeSet<String> {
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

pub(super) fn rail_items() -> serde_json::Value {
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

fn friend_rows() -> serde_json::Value {
	let person = |id: &str, name: &str, status: &str, subtitle: &str, relation: &str| {
		json!({
			"kind": "person", "key": format!("p{id}"), "id": id, "name": name, "username": name,
			"avatar": "", "status": status, "subtitle": subtitle, "relation": relation,
		})
	};
	json!([
		{ "kind": "title", "key": "tONLINE — 2", "title": "ONLINE — 2" },
		person("41", "ada", "online", "Online", "friend"),
		person("42", "grace", "dnd", "Playing Hades II", "friend"),
		{ "kind": "title", "key": "tINCOMING — 1", "title": "INCOMING — 1" },
		person("43", "linus", "offline", "Incoming Friend Request", "incoming"),
		person("44", "ken", "offline", "Outgoing Friend Request", "outgoing"),
		person("45", "troll", "offline", "Blocked", "blocked"),
	])
}

#[test]
fn the_friends_page_reaches_every_relationship_action() {
	let calls: Calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"the friends page",
		"ONLINE — 2",
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("sidebar.guild_id", json!("")),
			("home.page", json!("friends")),
			("friends.rows", friend_rows()),
		],
	);
	for (button, action, id) in [
		("friend_message_41", "friends.message", "41"),
		("friend_accept_43", "friends.accept", "43"),
		("friend_ignore_43", "friends.remove", "43"),
		("friend_cancel_44", "friends.remove", "44"),
		("friend_unblock_45", "friends.remove", "45"),
	] {
		calls.borrow_mut().clear();
		app.click(button);
		app.frames(2);
		assert_eq!(
			last_call(&calls, action),
			Some(json!(id)),
			"{button} calls {action}"
		);
		assert!(
			last_call(&calls, "friends.open_profile").is_none(),
			"{button} does not open the profile"
		);
	}
	for (item, action) in [
		("friend_remove_42", "friends.remove"),
		("friend_block_42", "friends.block"),
		("friend_copy_42", "friends.copy_id"),
	] {
		app.click("friend_more_42");
		app.frames(2);
		app.click(item);
		app.frames(2);
		assert_eq!(
			last_call(&calls, action),
			Some(json!("42")),
			"{item} calls {action}"
		);
	}
	app.click("friend_open_41");
	app.frames(2);
	let opened = last_call(&calls, "friends.open_profile").expect("a row opens the profile");
	assert_eq!(opened["id"], json!("41"));
	app.click("friends_tab_pending");
	app.frames(2);
	assert_eq!(last_call(&calls, "friends.select_tab"), Some(json!("pending")));
	app.type_text("friends_search", "gra");
	app.frames(2);
	assert_eq!(last_call(&calls, "friends.filter"), Some(json!("gra")));
	shows(
		&mut app,
		"the add friend tab",
		"ADD FRIEND",
		&[("friends.tab", json!("add"))],
	);
	app.type_text("friends_add_input", "grace");
	app.frames(2);
	app.click("friends_add_send");
	app.frames(2);
	assert_eq!(last_call(&calls, "friends.add"), Some(json!("grace")));
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

pub(super) fn sidebar_rows() -> serde_json::Value {
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

pub(super) fn member_window() -> serde_json::Value {
	json!({
		"first": 0,
		"rows": [
			{ "kind": "group", "key": "online", "title": "ONLINE — 1" },
			{
				"kind": "member", "key": "m30", "id": "30", "name": "ada", "extra": "ada_lovelace",
				"color": "#f47b67", "status": "online", "activity": "Playing Chess", "bot": false, "badge": "",
				"avatar": "",
			},
			{ "kind": "placeholder", "key": "p2", "index": 2 },
		],
	})
}
