use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::{Timeline, Zoom};
use media::{Gallery, Limits, MediaFetcher, MediaService};
use mockcord::{MockConfig, Server};
use protocol::{Snowflake, Token};
use session::{Command, Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Key, Modifiers, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{Prefs, Setup, TimelineView};

const WAIT: Duration = Duration::from_secs(20);
const WINDOW: (f32, f32) = (1280., 800.);
const SLOT: (f32, f32) = (760., 700.);
const BACKDROP: (f32, f32) = (640., 780.);

type Heard = Arc<Mutex<Vec<Changes>>>;
type Forwarded = Rc<RefCell<Vec<(Snowflake, Snowflake)>>>;

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

fn middle(app: &mut TestApp, id: &str) -> (f32, f32) {
	let bounds = app.bounds(id);
	(bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.)
}

fn click(app: &mut TestApp, (x, y): (f32, f32)) {
	let button = PointerButton::Primary;
	for event in [
		InputEvent::PointerMove { x, y },
		InputEvent::PointerDown { x, y, button },
		InputEvent::PointerUp { x, y, button },
	] {
		app.ui.dispatch_with_modifiers(event, Modifiers::default());
	}
	app.frames(4);
}

fn shown_name(view: &TimelineView) -> Option<String> {
	view.lightbox
		.viewing()
		.map(|viewing| viewing.current().name.clone())
}

#[test]
fn the_viewer_opens_on_a_tile_zooms_steps_and_closes() {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let server = tokio
		.block_on(Server::start(MockConfig::default()))
		.expect("mockcord starts");
	let root = scratch("viewer");
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
	let forwarded: Forwarded = Rc::default();
	let made: Rc<RefCell<Option<Rc<TimelineView>>>> = Rc::default();
	let (into, shown) = (made.clone(), made.clone());
	let (commander, handle, recorded) = (session.commander(), tokio.handle().clone(), forwarded.clone());
	let mut app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let fetcher = MediaFetcher::new().expect("a media fetcher");
			let service = MediaService::new(fetcher, scratch("viewer-media"), Limits::default());
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
			view.on_command(move |command: Command| commander.send(command));
			view.on_forward(move |message| recorded.borrow_mut().push(message));
			*into.borrow_mut() = Some(view);
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| {
			let view = shown.borrow().clone().expect("the timeline view");
			div()
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
					div()
						.absolute()
						.left(0.)
						.top(0.)
						.w(0.)
						.h(0.)
						.child(view.lightbox_view()),
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
	assert!(
		app.wait_until(|_| {
			pump(&session, &heard, &view, &timeline);
			timeline.borrow().messages().next().is_some()
		}),
		"the channel's history loads"
	);
	let message_id = server
		.post_images(
			channel_id,
			&[
				("first.png", (1920, 1080)),
				("second.png", (1080, 1920)),
				("third.png", (1000, 1000)),
			],
		)
		.expect("mockcord posts the images");
	let first_tile = || {
		timeline
			.borrow()
			.messages()
			.find(|entry| entry.id == message_id)
			.map(|entry| format!("image-tile-{}", entry.message.attachments[0].id))
	};
	assert!(
		app.wait_until(|app| {
			pump(&session, &heard, &view, &timeline);
			first_tile().is_some_and(|tile| app.find(&tile).is_some())
		}),
		"the posted images are laid out"
	);
	app.frames(10);
	let tile = first_tile().expect("the first tile");
	let on_tile = middle(&mut app, &tile);
	click(&mut app, on_tile);
	assert!(view.lightbox.is_open(), "clicking a tile opens the viewer");
	assert_eq!(shown_name(&view).as_deref(), Some("first.png"));
	assert!(app.find("viewer-image").is_some(), "the image is drawn");
	assert!(
		app.find("viewer-author").is_some(),
		"the author shows at the top left"
	);
	let author = timeline
		.borrow()
		.messages()
		.find(|entry| entry.id == message_id)
		.map(|entry| entry.author.clone())
		.expect("the message's author");
	assert!(app.texts().contains(&author), "the author's name shows");

	let zoom = middle(&mut app, "viewer-zoom");
	click(&mut app, zoom);
	assert_eq!(
		view.lightbox.zoom().scale,
		bridge::VIEWER_ZOOM_STEP,
		"the zoom button zooms in"
	);
	let image = middle(&mut app, "viewer-image");
	let before = app.bounds("viewer-image");
	let button = PointerButton::Primary;
	app.ui.dispatch(InputEvent::PointerDown {
		x: image.0,
		y: image.1,
		button,
	});
	app.ui.dispatch(InputEvent::PointerMove {
		x: image.0 + 60.,
		y: image.1 + 40.,
	});
	app.ui.dispatch(InputEvent::PointerUp {
		x: image.0 + 60.,
		y: image.1 + 40.,
		button,
	});
	app.frames(4);
	let after = app.bounds("viewer-image");
	assert!(
		after.x > before.x && after.y > before.y,
		"dragging pans the zoomed image: {before:?} to {after:?}"
	);
	assert!(
		view.lightbox.is_open(),
		"a drag on the image keeps the viewer open"
	);
	click(&mut app, zoom);
	assert_eq!(view.lightbox.zoom(), Zoom::FIT, "the zoom button zooms back out");
	let image = middle(&mut app, "viewer-image");
	app.ui.dispatch(InputEvent::Scroll {
		x: image.0,
		y: image.1,
		delta_x: 0.,
		delta_y: -300.,
	});
	app.frames(4);
	assert!(view.lightbox.zoom().zoomed(), "the wheel zooms in");
	click(&mut app, image);
	click(&mut app, image);
	assert_eq!(view.lightbox.zoom(), Zoom::FIT, "a double click resets the zoom");

	let next = middle(&mut app, "viewer-next");
	click(&mut app, next);
	assert_eq!(
		shown_name(&view).as_deref(),
		Some("second.png"),
		"Next shows the message's next image"
	);
	assert!(view.viewer_key(&Key::ArrowRight));
	assert_eq!(
		shown_name(&view).as_deref(),
		Some("third.png"),
		"the right arrow steps on"
	);
	assert!(view.viewer_key(&Key::ArrowRight));
	assert_eq!(
		shown_name(&view).as_deref(),
		Some("first.png"),
		"stepping wraps around"
	);
	assert!(view.viewer_key(&Key::ArrowLeft));
	assert_eq!(
		shown_name(&view).as_deref(),
		Some("third.png"),
		"the left arrow steps back"
	);

	let forward = middle(&mut app, "viewer-forward");
	click(&mut app, forward);
	assert_eq!(
		*forwarded.borrow(),
		[(channel_id, message_id)],
		"Forward hands the message on"
	);
	assert!(
		!view.lightbox.is_open(),
		"Forward closes the viewer for its dialog"
	);

	click(&mut app, on_tile);
	assert!(view.lightbox.is_open(), "the viewer opens again");
	let image = middle(&mut app, "viewer-image");
	click(&mut app, image);
	assert!(view.lightbox.is_open(), "a click on the image keeps it open");
	click(&mut app, BACKDROP);
	assert!(
		!view.lightbox.is_open(),
		"a click outside the image closes the viewer"
	);
	assert!(
		!view.viewer_key(&Key::ArrowRight),
		"a closed viewer takes no keys"
	);

	click(&mut app, on_tile);
	let close = middle(&mut app, "viewer-close");
	click(&mut app, close);
	assert!(!view.lightbox.is_open(), "Close closes the viewer");
}
