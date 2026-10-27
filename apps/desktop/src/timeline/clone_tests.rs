use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{Access, Inbox, MessageEntry, PluginMenuItem};
use media::{Gallery, Limits, MediaService};
use mockcord::{MockConfig, Server};
use plugins::{HostConfig, Permission, PluginHost, PluginRequest, Slot};
use protocol::fingerprint::Fingerprint;
use protocol::{Message, Snowflake, Token};
use serde_json::json;
use session::{Session, SessionConfig};
use state::{Expression, State};
use std::sync::Mutex;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::body::{self, Look};
use super::{Prefs, Setup, TimelineView};
use crate::community::{CommunityLayer, Dialog, Request};
use crate::plugin_menu::PluginMenu;

const CLONER: &str = "expression-cloner";
const EMOJI_ID: u64 = 4242;
const WAIT: Duration = Duration::from_secs(15);

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn open_session(server: &Server, root: &std::path::Path) -> Session {
	let paths = Paths::at(root.join("session")).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths).expect("settings save");
	let store = Store::open(paths).expect("store opens");
	Session::start(SessionConfig {
		identity: session::client_identity(&store),
		build_source: None,
		store: Arc::new(store),
		rest_base: url::Url::parse(server.rest_base_url()).expect("rest url"),
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		runtime: None,
		notify: bridge::changes_notifier(Inbox::new()),
		hooks: None,
		gateway: Default::default(),
		rest: Default::default(),
		caches: Default::default(),
		alerts: Default::default(),
		retention: Default::default(),
		remember_account: false,
		theme_links: Default::default(),
		trust: Default::default(),
	})
	.expect("session starts")
}

fn until(session: &Session, what: &str, ready: impl Fn(&State) -> bool) {
	let deadline = Instant::now() + WAIT;
	while !ready(&session.state().read()) {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		std::thread::sleep(Duration::from_millis(20));
	}
}

fn cloner(root: &std::path::Path) -> PluginHost {
	let host = PluginHost::new(HostConfig {
		data_dir: Some(root.join("plugins")),
		user_dir: None,
		development_dirs: Vec::new(),
		bundled: true,
		limits: plugins::Limits {
			load_deadline: WAIT,
			..plugins::Limits::default()
		},
	});
	let declared: Vec<Permission> = host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == CLONER)
		.map(|plugin| plugin.permissions)
		.expect("the bundled plugin is listed");
	host.approve(CLONER, &declared).expect("the plugin starts");
	host
}

fn menu_items(host: &PluginHost) -> Vec<PluginMenuItem> {
	host.menu_items(Slot::MessageMenu)
		.into_iter()
		.map(|item| PluginMenuItem {
			plugin: item.plugin,
			key: item.key,
			label: item.label,
			icon: item.icon.map(Into::into),
			when: item.when,
		})
		.collect()
}

fn press(app: &mut TestApp, id: &str, button: PointerButton) {
	let bounds = app.bounds(id);
	let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
	app.ui
		.input
		.dispatch(&app.ui.scene, InputEvent::PointerDown { x, y, button });
	app.ui
		.input
		.dispatch(&app.ui.scene, InputEvent::PointerUp { x, y, button });
	app.ui.prepare_frame();
	app.settle();
}

#[test]
fn right_clicking_an_emoji_then_clone_then_a_server_adds_it_there() {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let server = tokio
		.block_on(Server::start(MockConfig {
			record: true,
			..MockConfig::default()
		}))
		.expect("mockcord starts");
	let root = scratch("clone-flow");
	let session = {
		let _entered = tokio.enter();
		open_session(&server, &root)
	};
	let guild_id = Snowflake(server.fixtures().guilds[0].id);
	until(&session, "a server that takes expressions", |state| {
		state.may_create_expressions(guild_id)
			&& state
				.guild(guild_id)
				.is_some_and(|guild| !guild.channels.is_empty())
	});
	let host = cloner(&root);
	host.attach(session.state().clone(), session.commander());
	let heard = Arc::new(Mutex::new(Vec::new()));
	let sink = heard.clone();
	host.on_request(move |request| sink.lock().expect("requests").push(request));

	let origin = format!("http://{}", server.address());
	let cdn = server.cdn_url();
	let fingerprint = Fingerprint::web_client("en-US", 1);
	let fetcher =
		bridge::media_fetcher(&fingerprint, Some(server.address().port())).expect("a media fetcher");
	let loader = MediaService::new(fetcher, root.join("cache"), Limits::default());
	let gallery = Gallery::new(loader.clone(), tokio.handle().clone());
	let shared = session.state().clone();
	let (channel_id, message) = {
		let state = shared.read();
		let channel_id = state
			.guild(guild_id)
			.and_then(|guild| guild.channels.keys().next().copied())
			.expect("a channel");
		let message: Message = serde_json::from_value(json!({
			"id": "100",
			"channel_id": channel_id.to_string(),
			"guild_id": guild_id.to_string(),
			"author": { "id": "300", "username": "ada" },
			"content": format!("look <:blobwave:{EMOJI_ID}>"),
			"timestamp": "2026-10-01T12:00:00+00:00",
		}))
		.expect("the message decodes");
		(channel_id, message)
	};
	let entry: Rc<MessageEntry> = Rc::new(bridge::detached_entry(
		&shared.read(),
		Some(guild_id),
		&Arc::new(message),
	));
	let access = Access {
		guild_id: Some(guild_id),
		channel_id: Some(channel_id),
		..Access::default()
	};
	let activation = Rc::new(Cell::new(false));
	let ordered: Rc<RefCell<Vec<Request>>> = Rc::default();
	let layer: Rc<RefCell<Option<Rc<CommunityLayer>>>> = Rc::default();
	let built: Rc<RefCell<Option<View>>> = Rc::default();
	let (into, into_layer, orders) = (built.clone(), layer.clone(), ordered.clone());
	let (facts_state, gesture_host, pressed, allowed) = (
		shared.clone(),
		host.clone(),
		activation.clone(),
		activation.clone(),
	);
	let dialog_state = shared.clone();
	let items = menu_items(&host);
	let mut app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let view = TimelineView::new(
				runtime,
				Setup {
					gallery,
					cdn,
					mock_origin: Some(origin),
					code_font: "Consolas".to_owned(),
					prefs: Prefs::default(),
					store: None,
					developer_mode: false,
					pictures: None,
					display: bridge::Display::default(),
				},
			);
			let menu = PluginMenu::new();
			menu.set_items(items);
			menu.on_facts(move |target| bridge::MenuFacts::of(&facts_state.read(), target));
			menu.on_press(move |()| pressed.set(true));
			menu.on_choose(move |choice| {
				if !allowed.replace(false) {
					return;
				}
				gesture_host
					.menu_gesture(
						&choice.item.plugin,
						&choice.item.key,
						Slot::MessageMenu,
						choice.target,
						&[],
					)
					.expect("the plugin hears the click");
			});
			view.set_window(400., 300.);
			view.set_plugin_menu(menu);
			let community = CommunityLayer::new(runtime, view.pictures());
			community.attach(Some(dialog_state));
			community.on_request(move |request| orders.borrow_mut().push(request));
			let bump = runtime.signal(0);
			let names = bridge::Names::default();
			let document = markdown::parse(&entry.message.content);
			let rendered = body::render(
				&document,
				&view.env,
				&names,
				entry.id,
				0,
				&bump,
				Look::message(16.),
			);
			let (opener, opened) = (view.clone(), entry.clone());
			let row = div().id("row").w(360.).child(rendered).on_event(move |event| {
				if let InputEvent::PointerDown {
					x,
					y,
					button: PointerButton::Secondary,
				} = event.event && event.phase != zgx::input::EventPhase::Capture
				{
					opener.open_menu_at(&opened, access, (x, y));
				}
			});
			*into.borrow_mut() = Some(
				div()
					.w_full()
					.h_full()
					.child(row)
					.child(view.actions.context_menu())
					.child(community.view()),
			);
			*into_layer.borrow_mut() = Some(community);
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| built.borrow_mut().take().unwrap_or_else(div),
	);
	let emoji = format!("expression-{EMOJI_ID}");
	assert!(
		app.wait_until(|app| app.find(&emoji).is_some_and(|_| app.bounds(&emoji).width > 0.)),
		"the custom emoji is laid out"
	);
	press(&mut app, &emoji, PointerButton::Secondary);
	assert!(
		app.wait_until(|app| app.find("plugin-menu-clone_emoji").is_some()),
		"right-clicking a custom emoji offers Clone Emoji"
	);
	assert!(app.find("plugin-menu-clone_sticker").is_none());
	press(&mut app, "plugin-menu-clone_emoji", PointerButton::Primary);
	let expression = match heard.lock().expect("requests").pop() {
		Some(PluginRequest::Clone { expression, .. }) => expression,
		other => panic!("the plugin did not ask to clone: {other:?}"),
	};
	assert_eq!(
		expression,
		Expression::Emoji {
			id: Snowflake(EMOJI_ID),
			name: "blobwave".to_owned(),
			animated: false,
		}
	);
	let community = layer.borrow().clone().expect("the dialog layer");
	community.open(Dialog::Clone(expression));
	let row = format!("clone-server-{guild_id}");
	assert!(
		app.wait_until(|app| app.find(&row).is_some()),
		"the dialog lists the server"
	);
	app.frames(30);
	press(&mut app, &row, PointerButton::Primary);
	let order = ordered
		.borrow_mut()
		.pop()
		.expect("picking a server orders one clone");
	assert!(ordered.borrow().is_empty());
	assert!(!community.is_open(), "the dialog closes after one pick");
	let Request::Clone {
		expression,
		guild_id: target,
		name,
		..
	} = order
	else {
		panic!("the dialog asked for something else");
	};
	assert_eq!((target, name.as_str()), (guild_id, "blobwave"));
	let source = bridge::clone_source(&server.cdn_url(), &expression).expect("a source");
	let bytes = tokio
		.block_on(loader.bytes(&source))
		.expect("the emoji downloads")
		.to_vec();
	let command = bridge::clone_command(&shared.read(), &expression, target, &name, bytes);
	session.send(command);
	until(&session, "the clone in the server", |state| {
		state.guild(guild_id).is_some_and(|guild| {
			guild
				.emojis
				.iter()
				.any(|emoji| emoji.name.as_deref() == Some("blobwave"))
		})
	});
	session.shutdown();
	let _ = std::fs::remove_dir_all(&root);
}
