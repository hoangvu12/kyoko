use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::{Access, EmojiPicker, Timeline};
use chrono::Utc;
use media::{Gallery, Limits, MediaFetcher, MediaService};
use mockcord::{MockConfig, Server};
use protocol::{Snowflake, Token};
use session::{Command, Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{Prefs, Setup, TimelineView};

const WAIT: Duration = Duration::from_secs(20);
const WINDOW: (f32, f32) = (1280., 800.);
const SLOT: (f32, f32) = (760., 700.);
const REACT: &str = "message-action-add-reaction";
const AWAY: (f32, f32) = (1200., 780.);

type Heard = Arc<Mutex<Vec<Changes>>>;
type Reactions = Rc<RefCell<Vec<(Snowflake, Snowflake, String, bool)>>>;

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

fn pump(session: &Session, heard: &Heard, view: &TimelineView, timeline: &RefCell<Timeline>) {
	let batch: Vec<Changes> = std::mem::take(&mut *heard.lock().expect("the changes lock"));
	let mut changes = Changes::default();
	for item in batch {
		changes.merge(item);
	}
	let state = session.state().read();
	let mut timeline = timeline.borrow_mut();
	if timeline.apply(&state, &changes)
		&& let Some(delta) = timeline.take_delta()
	{
		view.apply(&timeline, &delta, "channel");
	}
}

fn point(app: &mut TestApp, (x, y): (f32, f32)) {
	app.ui.dispatch(InputEvent::PointerMove { x, y });
	app.frames(4);
}

fn middle(app: &mut TestApp, id: &str) -> (f32, f32) {
	let bounds = app.bounds(id);
	(bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.)
}

fn press(app: &mut TestApp, (x, y): (f32, f32), button: PointerButton) {
	for event in [
		InputEvent::PointerMove { x, y },
		InputEvent::PointerDown { x, y, button },
		InputEvent::PointerUp { x, y, button },
	] {
		app.ui.dispatch(event);
	}
	app.frames(4);
}

#[test]
fn the_reaction_picker_keeps_its_message_hovered_and_a_left_click_reacts() {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let server = tokio
		.block_on(Server::start(MockConfig::default()))
		.expect("mockcord starts");
	let root = scratch("react-hover");
	let heard: Heard = Arc::default();
	let session = {
		let _entered = tokio.enter();
		open_session(&server, &root, heard.clone())
	};
	let guild = &server.fixtures().guilds[0];
	let (guild_id, channel_id) = (Snowflake(guild.id), Snowflake(guild.message_channel_ids[0]));
	let deadline = Instant::now() + WAIT;
	while session.state().read().guild(guild_id).is_none() {
		assert!(Instant::now() < deadline, "the session never got its guilds");
		std::thread::sleep(Duration::from_millis(20));
	}
	let sent: Reactions = Rc::default();
	let made: Rc<RefCell<Option<Rc<TimelineView>>>> = Rc::default();
	let (into, shown) = (made.clone(), made.clone());
	let (commander, shared, handle, recorded) = (
		session.commander(),
		session.state().clone(),
		tokio.handle().clone(),
		sent.clone(),
	);
	let mut app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let fetcher = MediaFetcher::new().expect("a media fetcher");
			let service = MediaService::new(fetcher, scratch("react-hover-media"), Limits::default());
			let view = TimelineView::new(
				runtime,
				Setup {
					gallery: Gallery::new(service, handle),
					cdn: media::cdn::CDN.to_owned(),
					mock_origin: None,
					code_font: "Consolas".to_owned(),
					prefs: Prefs::default(),
					store: None,
					developer_mode: false,
					pictures: None,
					display: bridge::Display::default(),
				},
			);
			view.resize(SLOT.0, SLOT.1);
			view.set_window(WINDOW.0, WINDOW.1);
			view.on_command(move |command| {
				if let Command::AddReaction {
					channel_id,
					message_id,
					emoji,
					burst,
				} = &command
				{
					recorded
						.borrow_mut()
						.push((*channel_id, *message_id, emoji.clone(), *burst));
				}
				commander.send(command);
			});
			let catalog = shared.clone();
			view.on_emoji_catalog(move || Some(EmojiPicker::of(&catalog.read(), Some(guild_id))));
			view.set_access(Access::of(&shared.read(), Some(guild_id), channel_id, Utc::now()));
			*into.borrow_mut() = Some(view);
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| {
			let view = shown.borrow().clone().expect("the timeline view");
			div().w_full().h_full().child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.size(SLOT.0, SLOT.1)
					.child(view.view()),
			)
		},
	);
	app.ui.scene.borrow_mut().resize(WINDOW.0, WINDOW.1);
	let view = made.borrow().clone().expect("the timeline view");
	let timeline = RefCell::new(Timeline::default());
	let opened = timeline
		.borrow_mut()
		.select(&session.state().read(), Some(guild_id), channel_id);
	if let Some(command) = opened {
		session.send(command);
	}
	let newest = || {
		timeline
			.borrow()
			.messages()
			.rev()
			.find(|entry| !entry.pending && !entry.message.content.is_empty() && bridge::has_actions(entry))
			.map(|entry| entry.id)
	};
	assert!(
		app.wait_until(|app| {
			pump(&session, &heard, &view, &timeline);
			newest().is_some_and(|id| app.find(&format!("message-text-{id}")).is_some())
		}),
		"the channel's newest message is laid out"
	);
	let message_id = newest().expect("a message");
	let text = format!("message-text-{message_id}");
	app.frames(10);
	let over = middle(&mut app, &text);
	point(&mut app, over);
	assert!(
		app.find(REACT).is_some(),
		"hovering the message shows its toolbar"
	);
	let react = middle(&mut app, REACT);
	press(&mut app, react, PointerButton::Primary);
	app.frames(30);
	assert!(view.actions.picker.is_open(), "Add Reaction opens the picker");
	let cell = EmojiPicker::of(&session.state().read(), Some(guild_id))
		.sections(0)
		.into_iter()
		.flat_map(|section| section.cells)
		.find(|cell| cell.custom.is_some() && !cell.locked)
		.expect("a usable custom emoji");
	let cell_id = format!("picker-emoji-{}", cell.key);
	let on_cell = middle(&mut app, &cell_id);
	point(&mut app, on_cell);
	assert!(
		app.find(REACT).is_some(),
		"the message keeps its toolbar while its picker is open"
	);
	press(&mut app, on_cell, PointerButton::Primary);
	assert!(
		sent.borrow()
			.contains(&(channel_id, message_id, cell.reaction.clone(), false)),
		"a left click reacts with the emoji: {:?}",
		sent.borrow()
	);
	assert!(!view.actions.picker.is_open(), "reacting closes the picker");
	point(&mut app, AWAY);
	assert!(
		app.find(REACT).is_none(),
		"the toolbar goes once the picker closed and the pointer left"
	);
}
