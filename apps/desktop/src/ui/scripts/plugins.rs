use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use plugins::{HostConfig, Limits, PluginHost};
use serde_json::json;
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::{WIDE, host_names, server_values, shows};
use crate::theme::Catalog;
use crate::ui::plugin_slots::{PluginSlots, Request};
use crate::ui::plugins::PluginsView;
use crate::ui::{themes_dir, ui_dir};

pub(super) const PROBE: &str = "slot-probe";
const LOAD_DEADLINE: Duration = Duration::from_secs(15);
pub(super) const PROBE_MARKERS: [&str; 3] = ["probe in composer", "probe in panel", "probe in rail"];
pub(super) const ALL_SLOTS: [&str; 3] = ["ui:composer.buttons", "ui:panel.buttons", "ui:rail.buttons"];
const PROBE_SETTINGS: &str = "[[settings]]\nkey = \"greeting\"\ntype = \"string\"\nlabel = \"Greeting\"\ndefault = \"hello from the probe\"\n";
const PROBE_UI: &str = r#"
local settings = ui.shared("plugin.slot-probe.settings", {})

local probe = ui.component(function(props: any)
	local area = props.slot.area
	return ui.button {
		id = "probe_" .. area,
		on_click = function()
			local slot = if area == "panel" then "rail.buttons" else "composer.buttons"
			ui.invoke("plugin.slot-probe.action", { key = "ping", slot = slot })
		end,
		ui.text { "probe in " .. area },
		ui.text { function()
			return settings:get().greeting or ""
		end },
	}
end)

ui.contribute("composer.buttons", probe {})
ui.contribute("panel.buttons", probe {})
ui.contribute("rail.buttons", probe {})
"#;

const BROKEN: [(&str, &str); 7] = [
	(
		"syntax-error",
		"ui.contribute(\"composer.buttons\", ui.text { \"never shown\" ",
	),
	("load-error", "error(\"this plugin fails while it loads\")"),
	(
		"build-error",
		"local broken = ui.component(function() error(\"this plugin fails while it builds\") end)\nui.contribute(\"composer.buttons\", broken {})",
	),
	(
		"app-reach",
		"local open = ui.shared(\"composer.open\", false)\nui.contribute(\"composer.buttons\", ui.text { \"reached the app\" })",
	),
	(
		"neighbour-reach",
		"local other = ui.shared(\"plugin.slot-probe.settings\", {})\nui.contribute(\"composer.buttons\", ui.text { \"reached a neighbour\" })",
	),
	(
		"network-reach",
		"ui.fetch(\"http://127.0.0.1:9/\")\nui.contribute(\"composer.buttons\", ui.text { \"reached the network\" })",
	),
	(
		"action-reach",
		"ui.invoke(\"login.sign_out\")\nui.contribute(\"composer.buttons\", ui.text { \"reached an app action\" })",
	),
];

pub(super) struct Lab {
	root: PathBuf,
	pub host: PluginHost,
	pub slots: Option<PluginSlots>,
	pub view: Option<PluginsView>,
	errors: Rc<RefCell<Vec<String>>>,
}

impl Drop for Lab {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.root);
	}
}

impl Lab {
	pub(super) fn new(name: &str, extra_dirs: &[PathBuf]) -> Lab {
		let root = std::env::temp_dir().join(format!("kyoko-plugin-ui-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&root);
		std::fs::create_dir_all(root.join("dev")).expect("scratch dir");
		let mut development_dirs = vec![root.join("dev")];
		development_dirs.extend_from_slice(extra_dirs);
		let host = PluginHost::new(HostConfig {
			data_dir: Some(root.join("data")),
			user_dir: None,
			development_dirs,
			bundled: false,
			limits: Limits {
				load_deadline: LOAD_DEADLINE,
				..Limits::default()
			},
		});
		Lab {
			root,
			host,
			slots: None,
			view: None,
			errors: Rc::default(),
		}
	}

	pub(super) fn write(&self, id: &str, permissions: &[&str], extra: &str, ui: &str) {
		let dir = self.root.join("dev").join(id);
		std::fs::create_dir_all(&dir).expect("plugin dir");
		let permissions: Vec<String> = permissions
			.iter()
			.map(|permission| format!("{permission:?}"))
			.collect();
		let manifest = format!(
			"id = {id:?}\nname = {id:?}\nversion = \"1.0.0\"\napi = 1\npermissions = [{}]\n{extra}\n",
			permissions.join(", ")
		);
		std::fs::write(dir.join("plugin.toml"), manifest).expect("manifest");
		std::fs::write(dir.join("ui.luau"), ui).expect("ui");
	}

	pub(super) fn probe(name: &str, permissions: &[&str]) -> Lab {
		let lab = Lab::new(name, &[]);
		lab.write(PROBE, permissions, PROBE_SETTINGS, PROBE_UI);
		lab
	}

	pub(super) fn approve_all(&self) {
		self.host.scan();
		for plugin in self.host.snapshot() {
			self.host
				.approve(&plugin.id, &plugin.permissions)
				.expect("the plugin is approved");
		}
	}

	pub(super) fn mount(&mut self, script: &ScriptHost, handler: impl Fn(Request) + 'static) {
		let slots = PluginSlots::new(script, handler);
		slots.publish(script, &self.host.snapshot());
		self.slots = Some(slots);
	}

	fn setting(&self, id: &str, key: &str) -> serde_json::Value {
		self.host
			.snapshot()
			.into_iter()
			.find(|plugin| plugin.id == id)
			.and_then(|plugin| plugin.values.get(key).cloned())
			.unwrap_or_default()
	}
}

pub(super) fn app_with(
	lab: Lab,
	mount: impl FnOnce(&mut Lab, &ScriptHost) + 'static,
) -> (TestApp, Rc<RefCell<Lab>>) {
	let lab = Rc::new(RefCell::new(lab));
	let held = lab.clone();
	let app = TestApp::new(move |runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, crate::theme::DEFAULT);
		let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
		for name in host_names() {
			host.action(name.clone(), |_| {});
			host.function(name, |_| Value::Nil);
		}
		let errors = held.borrow().errors.clone();
		host.on_error(move |message| errors.borrow_mut().push(message.to_owned()));
		mount(&mut held.borrow_mut(), &host);
		host
	});
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	(app, lab)
}

pub(super) fn channel_values() -> Vec<(&'static str, serde_json::Value)> {
	let mut values = server_values(false);
	values.push(("composer.open", json!(true)));
	values.push((
		"user.panel",
		json!({ "name": "ada", "subtitle": "ada", "emoji": "", "emoji_image": "", "status": "online", "latency": "" }),
	));
	values
}

fn shows_despite_errors(app: &mut TestApp, screen: &str, marker: &str, values: &[(&str, serde_json::Value)]) {
	for (key, value) in values {
		let value = Value::from_json(&value.to_string()).unwrap_or(Value::Nil);
		app.host.shared(key, Value::Nil).set(value);
	}
	app.frames(2);
	let texts = app.texts();
	assert!(
		texts.iter().any(|text| text == marker),
		"{screen} does not show \"{marker}\"; it shows {texts:?}"
	);
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
	let deadline = Instant::now() + Duration::from_secs(5);
	while !done() {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		std::thread::sleep(Duration::from_millis(10));
	}
}

fn rules_of(lab: &Rc<RefCell<Lab>>) -> serde_json::Value {
	lab.borrow().setting("text-replace", "rules")
}

#[test]
fn the_rules_editor_adds_edits_and_removes_rows() {
	let plugin_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/text-replace");
	let lab = Lab::new("text-replace", &[plugin_dir]);
	lab.host.scan();
	let shown = lab
		.host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == "text-replace")
		.expect("TextReplace loads")
		.permissions;
	lab.host.approve("text-replace", &shown).expect("approved");
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(1)
		.enable_all()
		.build()
		.expect("a runtime");
	let handle = runtime.handle().clone();
	let (mut app, lab) = app_with(lab, move |lab, host| {
		lab.view = Some(PluginsView::new(host, lab.host.clone(), handle));
		let plugins = lab.host.clone();
		host.action("plugins.set", move |argument| {
			let argument: serde_json::Value = serde_json::from_str(&argument.to_json()).unwrap_or_default();
			let (Some(id), Some(key)) = (argument["id"].as_str(), argument["key"].as_str()) else {
				return;
			};
			if let Err(error) = plugins.set_setting(id, key, &argument["value"]) {
				panic!("the editor sent a setting the host refused: {error}");
			}
		});
	});
	shows(
		&mut app,
		"the plugins page",
		"TextReplace",
		&[
			("login.screen", json!("main")),
			("settings.values", super::setting_values(true)),
			("settings.page", json!("plugins")),
		],
	);
	app.click("plugin_settings_text-replace");
	shows(&mut app, "TextReplace's settings", "Nothing added yet.", &[]);
	assert_eq!(rules_of(&lab), json!([]));

	app.click("plugin_list_rules_add");
	shows(&mut app, "the first rule", "Rule 1", &[]);
	assert_eq!(
		rules_of(&lab),
		json!([{ "find": "", "replace": "", "only_if": "", "pattern": false }])
	);
	app.type_text("plugin_list_rules_1_find", "teh");
	app.type_text("plugin_list_rules_1_replace", "the");
	wait_for("the typed rule to be saved", || {
		app.frames(1);
		rules_of(&lab)[0]["replace"] == json!("the")
	});
	app.click("plugin_list_rules_1_pattern");

	app.click("plugin_list_rules_add");
	shows(&mut app, "the second rule", "Rule 2", &[]);
	app.type_text("plugin_list_rules_2_find", "cya");
	app.click("plugin_list_rules_2_up");
	app.frames(2);
	assert_eq!(
		rules_of(&lab),
		json!([
			{ "find": "cya", "replace": "", "only_if": "", "pattern": false },
			{ "find": "teh", "replace": "the", "only_if": "", "pattern": true },
		]),
		"moving a row up saves the new order, with what was typed into it"
	);

	app.click("plugin_list_rules_1_remove");
	app.frames(2);
	assert_eq!(
		rules_of(&lab),
		json!([{ "find": "teh", "replace": "the", "only_if": "", "pattern": true }])
	);
	assert!(
		app.find("plugin_list_rules_2_remove").is_none(),
		"the removed row is gone"
	);
	assert!(
		!app.texts().iter().any(|text| text == "Rule 2"),
		"one rule is left"
	);
	drop(app);
	drop(runtime);
}

#[test]
fn a_plugin_shows_only_in_the_slots_it_was_granted() {
	let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
	let seen = requests.clone();
	let lab = Lab::probe("granted", &["ui:composer.buttons", "ui:panel.buttons"]);
	lab.approve_all();
	let (mut app, _lab) = app_with(lab, move |lab, host| {
		lab.mount(host, move |request| seen.borrow_mut().push(request))
	});
	shows(
		&mut app,
		"a channel with a plugin",
		"probe in composer",
		&channel_values(),
	);
	let texts = app.texts();
	assert!(texts.iter().any(|text| text == "probe in panel"));
	assert!(
		texts.iter().any(|text| text == "hello from the probe"),
		"the plugin reads its own settings: {texts:?}"
	);
	assert!(
		!texts.iter().any(|text| text == "probe in rail"),
		"a slot the plugin was not granted shows nothing of it"
	);

	app.click("probe_composer");
	app.click("probe_panel");
	assert_eq!(
		requests.borrow().as_slice(),
		[
			Request::Action {
				id: PROBE.to_owned(),
				key: "ping".to_owned(),
				slot: Some("composer.buttons"),
			},
			Request::Action {
				id: PROBE.to_owned(),
				key: "ping".to_owned(),
				slot: None,
			},
		],
		"a click reaches the host as the plugin's own action, naming only a slot it was granted"
	);
}

#[test]
fn a_broken_or_overreaching_plugin_interface_leaves_the_shell_building() {
	let lab = Lab::probe("broken", &ALL_SLOTS);
	for (id, source) in BROKEN {
		lab.write(id, &["ui:composer.buttons"], "", source);
	}
	lab.approve_all();
	let (mut app, lab) = app_with(lab, |lab, host| lab.mount(host, |_| {}));
	shows_despite_errors(
		&mut app,
		"a channel beside broken plugins",
		"general",
		&channel_values(),
	);
	let texts = app.texts();
	for marker in PROBE_MARKERS {
		assert!(
			texts.iter().any(|text| text == marker),
			"the working plugin still shows \"{marker}\": {texts:?}"
		);
	}
	for text in [
		"never shown",
		"reached the app",
		"reached a neighbour",
		"reached the network",
		"reached an app action",
	] {
		assert!(
			!texts.iter().any(|shown| shown == text),
			"\"{text}\" must not show"
		);
	}
	assert!(
		app.find("channel_20").is_some() && app.find("probe_composer").is_some(),
		"the sidebar and the composer still build"
	);
	let errors = lab.borrow().errors.borrow().join(
		"
",
	);
	for (id, _) in BROKEN {
		assert!(
			errors.contains(&format!("{id}:")),
			"{id} is reported as broken: {errors}"
		);
	}
}

#[test]
fn the_silent_typing_button_switches_its_setting_and_unmounts_with_the_plugin() {
	let plugins_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
	let lab = Lab::new("silent-typing", &[plugins_dir]);
	lab.host.scan();
	let shown = lab
		.host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == "silent-typing")
		.expect("SilentTyping loads")
		.permissions;
	lab.host.approve("silent-typing", &shown).expect("approved");
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(1)
		.enable_all()
		.build()
		.expect("a runtime");
	let handle = runtime.handle().clone();
	let (mut app, lab) = app_with(lab, move |lab, host| {
		lab.view = Some(PluginsView::new(host, lab.host.clone(), handle));
	});
	shows(
		&mut app,
		"a channel with SilentTyping",
		"general",
		&channel_values(),
	);
	assert!(
		app.find("silent_typing_composer").is_some(),
		"the toggle is in the composer"
	);
	assert_eq!(lab.borrow().setting("silent-typing", "silent"), json!(true));

	app.click("silent_typing_composer");
	std::thread::sleep(Duration::from_millis(200));
	assert_eq!(
		lab.borrow().setting("silent-typing", "silent"),
		json!(true),
		"without a real click or key press the button changes nothing"
	);

	lab.borrow().view.as_ref().expect("the view").activated();
	app.click("silent_typing_composer");
	wait_for("the click to switch SilentTyping off", || {
		lab.borrow().setting("silent-typing", "silent") == json!(false)
	});
	assert!(
		!lab.borrow().view.as_ref().expect("the view").publish(),
		"a changed setting republishes without remounting"
	);

	lab.borrow()
		.host
		.set_setting("silent-typing", "button", &json!(false))
		.expect("accepted");
	lab.borrow().view.as_ref().expect("the view").publish();
	app.frames(2);
	assert!(
		app.find("silent_typing_composer").is_none(),
		"the button follows its setting"
	);

	lab.borrow()
		.host
		.set_setting("silent-typing", "button", &json!(true))
		.expect("accepted");
	lab.borrow().host.disable("silent-typing");
	{
		let lab = lab.borrow();
		let view = lab.view.as_ref().expect("the view");
		assert!(view.publish(), "disabling unmounts the interface half");
		view.remount();
	}
	app.frames(2);
	assert!(
		app.find("silent_typing_composer").is_none(),
		"a disabled plugin shows nothing"
	);
	shows(&mut app, "the channel without SilentTyping", "general", &[]);

	lab.borrow().host.enable("silent-typing").expect("enabled");
	{
		let lab = lab.borrow();
		let view = lab.view.as_ref().expect("the view");
		assert!(view.publish(), "enabling mounts it again");
		view.remount();
	}
	app.frames(2);
	assert!(app.find("silent_typing_composer").is_some(), "the toggle is back");
	drop(app);
	drop(runtime);
}

fn mock_session(server: &mockcord::Server, root: &Path) -> session::Session {
	let paths = store::Paths::at(root.join("session")).expect("scratch paths");
	let mut settings = store::Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths).expect("settings save");
	let store = store::Store::open(paths).expect("store opens");
	session::Session::start(session::SessionConfig {
		identity: session::client_identity(&store),
		build_source: None,
		store: std::sync::Arc::new(store),
		rest_base: url::Url::parse(server.rest_base_url()).expect("rest url"),
		gateway_url: server.gateway_url().to_owned(),
		token: protocol::Token::new(server.token()),
		runtime: None,
		notify: bridge::changes_notifier(bridge::Inbox::new()),
		hooks: None,
		gateway: Default::default(),
		rest: Default::default(),
		caches: Default::default(),
		alerts: Default::default(),
		retention: Default::default(),
		remember_account: false,
		theme_links: Default::default(),
	})
	.expect("session starts")
}

fn anything_unread(state: &state::State) -> bool {
	state
		.guild_summaries()
		.iter()
		.any(|guild| guild.unread || guild.mentions > 0)
		|| state.dm_rows().iter().any(|dm| dm.unread || dm.mentions > 0)
}

#[test]
fn the_read_all_button_on_the_rail_marks_everything_read_only_when_clicked() {
	const READ_ALL: &str = "read-all-notifications";
	const BUTTON: &str = "read_all_notifications_rail";
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.expect("a runtime");
	let server = runtime
		.block_on(mockcord::Server::start(mockcord::MockConfig {
			record: true,
			..mockcord::MockConfig::default()
		}))
		.expect("mockcord starts");
	let plugins_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
	let lab = Lab::new("read-all", &[plugins_dir]);
	let session = {
		let _entered = runtime.enter();
		mock_session(&server, &lab.root)
	};
	wait_for("the mock session to load unread servers", || {
		let state = session.state().read();
		state.session().connected && anything_unread(&state)
	});
	lab.host.scan();
	lab.host.attach(session.state().clone(), session.commander());
	let shown = lab
		.host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == READ_ALL)
		.expect("ReadAllNotificationsButton loads")
		.permissions;
	lab.host.approve(READ_ALL, &shown).expect("approved");
	let handle = runtime.handle().clone();
	let (mut app, lab) = app_with(lab, move |lab, host| {
		lab.view = Some(PluginsView::new(host, lab.host.clone(), handle));
	});
	shows(
		&mut app,
		"a server with the read-all button",
		"general",
		&channel_values(),
	);
	assert!(app.find(BUTTON).is_some(), "the button is on the server rail");
	let acks = || {
		server
			.control()
			.recording()
			.requests_to("POST", "/read-states/ack-bulk")
			.count()
	};

	app.click(BUTTON);
	std::thread::sleep(Duration::from_millis(500));
	assert_eq!(acks(), 0, "without a real click or key press nothing is sent");
	assert!(anything_unread(&session.state().read()));

	lab.borrow().view.as_ref().expect("the view").activated();
	app.click(BUTTON);
	wait_for("the click to mark every server read", || {
		!anything_unread(&session.state().read()) && acks() > 0
	});
	assert_eq!(
		server
			.control()
			.recording()
			.requests_to("POST", "/channels/*/messages/*/ack")
			.count(),
		0,
		"the click goes through the bulk ack path"
	);

	lab.borrow().host.disable(READ_ALL);
	{
		let lab = lab.borrow();
		let view = lab.view.as_ref().expect("the view");
		assert!(view.publish(), "disabling unmounts the button");
		view.remount();
	}
	app.frames(2);
	assert!(app.find(BUTTON).is_none(), "the rail is back to Home alone");
	drop(app);
	drop(session);
	drop(server);
	drop(runtime);
}
