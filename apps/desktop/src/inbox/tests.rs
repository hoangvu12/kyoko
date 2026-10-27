use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::{InboxTab, Timeline, today};
use media::{Gallery, Limits, MediaFetcher, MediaService};
use mockcord::{MockConfig, Server};
use protocol::{Snowflake, Token};
use session::{Command, Session, SessionConfig};
use state::{Changes, State};
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Key, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{InboxLayer, InboxShown, Request};
use crate::timeline::{Pictures, Prefs, Setup, TimelineView};

const WAIT: Duration = Duration::from_secs(20);
const WINDOW: (f32, f32) = (1280., 800.);
const SLOT: (f32, f32) = (760., 640.);
const ANCHOR: (f32, f32) = (1137., 37.);
const OUTSIDE: (f32, f32) = (200., 500.);
const BUTTON: f32 = 32.;

type Heard = Arc<Mutex<Vec<Changes>>>;

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn open_session(server: &Server, root: &std::path::Path, heard: Heard) -> Session {
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
		notify: Arc::new(move |changes| heard.lock().expect("the changes lock").push(changes)),
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

struct Parts {
	layer: Rc<InboxLayer>,
	view: Rc<TimelineView>,
	timeline: RefCell<Timeline>,
	heard: Vec<Request>,
}

fn press(app: &mut TestApp, (x, y): (f32, f32)) {
	for event in [
		InputEvent::PointerDown {
			x,
			y,
			button: PointerButton::Primary,
		},
		InputEvent::PointerUp {
			x,
			y,
			button: PointerButton::Primary,
		},
	] {
		app.ui.dispatch(event);
	}
	app.frames(2);
}

fn pump(session: &Session, heard: &Heard, parts: &RefCell<Option<Parts>>) {
	let batch: Vec<Changes> = std::mem::take(&mut *heard.lock().expect("the changes lock"));
	let mut changes = Changes::default();
	for item in batch {
		changes.merge(item);
	}
	let parts = parts.borrow();
	let Some(parts) = parts.as_ref() else {
		return;
	};
	let state = session.state().read();
	if parts.layer.is_open() && (changes.recent_mentions || changes.guild_list) {
		parts.layer.show(shown(&state));
	}
	let mut timeline = parts.timeline.borrow_mut();
	if timeline.apply(&state, &changes) {
		publish(&parts.view, &mut timeline);
	}
}

fn publish(view: &TimelineView, timeline: &mut Timeline) {
	if let Some(delta) = timeline.take_delta() {
		view.apply(timeline, &delta, "channel");
	}
}

fn shown(state: &State) -> InboxShown {
	InboxShown {
		mentions: bridge::mention_groups(state, today()),
		..InboxShown::default()
	}
}

fn first_mention(state: &State) -> Option<(Option<Snowflake>, Snowflake, Snowflake)> {
	let message = state.recent_mentions()?.first()?.clone();
	let guild_id = message
		.guild_id
		.or_else(|| state.channel_guild(message.channel_id));
	Some((guild_id, message.channel_id, message.id))
}

#[test]
fn jump_opens_the_mentioned_message_and_escape_or_an_outside_press_closes_the_inbox() {
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
	let root = scratch("inbox-jump");
	let heard: Heard = Arc::default();
	let session = {
		let _entered = tokio.enter();
		open_session(&server, &root, heard.clone())
	};
	let deadline = Instant::now() + WAIT;
	while session.state().read().guilds().next().is_none() {
		assert!(Instant::now() < deadline, "the session never got its guilds");
		std::thread::sleep(Duration::from_millis(20));
	}
	let parts: Rc<RefCell<Option<Parts>>> = Rc::default();
	let (into, requested) = (parts.clone(), parts.clone());
	let commander = session.commander();
	let shared = session.state().clone();
	let handle = tokio.handle().clone();
	let built: Rc<RefCell<Option<View>>> = Rc::default();
	let view_slot = built.clone();
	let mut app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let fetcher = MediaFetcher::new().expect("a media fetcher");
			let service = MediaService::new(fetcher, scratch("inbox-jump-media"), Limits::default());
			let gallery = Gallery::new(service, handle);
			let pictures = Rc::new(Pictures::new(
				runtime.clone(),
				gallery.clone(),
				media::cdn::CDN.to_owned(),
				None,
			));
			let view = TimelineView::new(
				runtime,
				Setup {
					gallery,
					cdn: media::cdn::CDN.to_owned(),
					mock_origin: None,
					code_font: "Consolas".to_owned(),
					prefs: Prefs::default(),
					store: None,
					developer_mode: false,
					pictures: Some(pictures.clone()),
					display: bridge::Display::default(),
				},
			);
			view.resize(SLOT.0, SLOT.1);
			view.set_window(WINDOW.0, WINDOW.1);
			let sender = commander.clone();
			view.on_command(move |command| sender.send(command));
			let layer = InboxLayer::new(runtime, pictures);
			layer.set_window(WINDOW.0, WINDOW.1);
			let (sender, handled) = (commander.clone(), requested.clone());
			layer.on_request(move |request| {
				if let Request::Load(InboxTab::Mentions, filter) = &request {
					sender.send(Command::LoadRecentMentions {
						everyone: filter.everyone,
						roles: filter.roles,
						guild_id: None,
					});
				}
				if let Request::JumpTo {
					guild_id,
					channel_id,
					message_id,
				} = &request && let Some(parts) = handled.borrow().as_ref()
				{
					let state = shared.read();
					let command = parts.timeline.borrow_mut().select(&state, *guild_id, *channel_id);
					publish(&parts.view, &mut parts.timeline.borrow_mut());
					if let Some(command) = command {
						sender.send(command);
					}
					parts.view.jump_to(*message_id);
				}
				if let Some(parts) = handled.borrow_mut().as_mut() {
					parts.heard.push(request);
				}
			});
			let (escaping, opener) = (layer.clone(), layer.clone());
			*view_slot.borrow_mut() = Some(
				overlay()
					.w_full()
					.h_full()
					.child(
						div()
							.absolute()
							.left(0.)
							.top(0.)
							.size(SLOT.0, SLOT.1)
							.child(view.view()),
					)
					.child(
						button()
							.id("inbox-button")
							.absolute()
							.left(ANCHOR.0 - BUTTON / 2.)
							.top(ANCHOR.1 - BUTTON / 2.)
							.size(BUTTON, BUTTON)
							.on_click(move || opener.toggle(ANCHOR)),
					)
					.child(div().absolute().left(0.).top(0.).w(0.).h(0.).child(layer.view()))
					.on_event(move |event| {
						if let InputEvent::KeyDown { key: Key::Escape, .. } = event.event
							&& escaping.escape()
						{
							event.stop_propagation();
						}
					}),
			);
			*into.borrow_mut() = Some(Parts {
				layer,
				view,
				timeline: RefCell::new(Timeline::default()),
				heard: Vec::new(),
			});
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| built.borrow_mut().take().unwrap_or_else(div),
	);
	app.ui.scene.borrow_mut().resize(WINDOW.0, WINDOW.1);
	let layer = parts.borrow().as_ref().expect("the parts").layer.clone();

	app.click("inbox-button");
	let waited = app.wait_until(|app| {
		pump(&session, &heard, &parts);
		app.find("inbox-popout").is_some() && first_mention(&session.state().read()).is_some()
	});
	assert!(waited, "opening the inbox fetches the recent mentions");
	let (guild_id, channel_id, message_id) =
		first_mention(&session.state().read()).expect("a recent mention");
	let jump = format!("inbox-jump-{message_id}");
	assert!(
		app.wait_until(|app| {
			pump(&session, &heard, &parts);
			app.find(&jump).is_some()
		}),
		"the newest mention shows with a Jump button"
	);
	app.frames(10);
	let card = app.bounds(&format!("inbox-card-{message_id}"));
	app.ui.dispatch(InputEvent::PointerMove {
		x: card.x + card.width / 2.,
		y: card.y + card.height / 2.,
	});
	app.frames(4);
	app.click(&jump);
	app.frames(2);
	assert!(!layer.is_open(), "Jump closes the inbox");
	assert!(app.find("inbox-popout").is_none(), "the popout leaves the window");
	assert!(
		parts
			.borrow()
			.as_ref()
			.expect("the parts")
			.heard
			.contains(&Request::JumpTo {
				guild_id,
				channel_id,
				message_id,
			}),
		"Jump asks for the mentioned message in its channel"
	);
	let row = format!("message-text-{message_id}");
	let opened = app.wait_until(|app| {
		pump(&session, &heard, &parts);
		let parts = parts.borrow();
		let parts = parts.as_ref().expect("the parts");
		parts.timeline.borrow().open().map(|open| open.channel_id) == Some(channel_id)
			&& parts
				.timeline
				.borrow()
				.messages()
				.any(|entry| entry.id == message_id)
			&& app.find(&row).is_some()
	});
	assert!(opened, "the mentioned message is open in the timeline");

	app.click("inbox-button");
	assert!(
		app.wait_until(|app| app.find("inbox-popout").is_some()),
		"the inbox button opens it again"
	);
	app.key(Key::Escape);
	app.frames(2);
	assert!(!layer.is_open(), "Escape closes the inbox");

	app.click("inbox-button");
	assert!(
		app.wait_until(|app| app.find("inbox-popout").is_some()),
		"the inbox opens a third time"
	);
	press(&mut app, OUTSIDE);
	assert!(!layer.is_open(), "a press outside closes the inbox");
	session.shutdown();
	let _ = std::fs::remove_dir_all(&root);
}
