use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::Timeline;
use media::{Gallery, Limits, MediaService, Update};
use mockcord::{MockConfig, Server};
use protocol::fingerprint::Fingerprint;
use protocol::{Snowflake, Token};
use session::{Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{Prefs, Setup, TimelineView};

const WAIT: Duration = Duration::from_secs(30);
const WINDOW: (f32, f32) = (1280., 800.);
const SLOT: (f32, f32) = (760., 700.);
const GRIDS: usize = 4;
const GRID_IMAGES: usize = 10;

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

fn show(gallery: &Gallery, view: &TimelineView) {
	let visible = view.visible_images();
	gallery.set_visible(visible.iter().map(media::ImageName::as_str));
	for update in gallery.drain(Instant::now()) {
		match update {
			Update::Show { name, pixels } => {
				let image = ImageData::new(pixels.width, pixels.height, pixels.rgba)
					.ok()
					.map(Arc::new);
				view.show_image(&name, image);
			}
			Update::Clear { name } => view.show_image(&name, None),
		}
	}
}

#[test]
fn a_channel_full_of_image_grids_fills_its_screen_over_one_multiplexed_connection() {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(4)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let server = tokio
		.block_on(Server::start(MockConfig {
			record: true,
			media_delay: MockConfig::CROWDED_MEDIA_DELAY,
			..MockConfig::default()
		}))
		.expect("mockcord starts");
	let root = scratch("image-channel");
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
	for grid in 0..GRIDS {
		let names: Vec<String> = (0..GRID_IMAGES)
			.map(|index| format!("grid-{grid}-{index}.png"))
			.collect();
		let images: Vec<(&str, (u32, u32))> = names
			.iter()
			.enumerate()
			.map(|(index, name)| (name.as_str(), (1200 + 40 * index as u32, 900)))
			.collect();
		server
			.post_images(channel_id, &images)
			.expect("mockcord posts the images");
	}
	let fingerprint = Fingerprint::web_client("en-US", 1);
	let fetcher =
		bridge::media_fetcher(&fingerprint, Some(server.address().port())).expect("a media fetcher");
	let gallery = Gallery::new(
		MediaService::new(fetcher, root.join("cache"), Limits::default()),
		tokio.handle().clone(),
	);
	let made: Rc<RefCell<Option<Rc<TimelineView>>>> = Rc::default();
	let (into, shown) = (made.clone(), made.clone());
	let (view_gallery, origin, cdn) = (
		gallery.clone(),
		format!("http://{}", server.address()),
		server.cdn_url(),
	);
	let app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let view = TimelineView::new(
				runtime,
				Setup {
					gallery: view_gallery,
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
			view.resize(SLOT.0, SLOT.1);
			view.set_window(WINDOW.0, WINDOW.1);
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
	let started = Instant::now();
	let opened = timeline
		.borrow_mut()
		.select(&session.state().read(), Some(guild_id), channel_id);
	if let Some(command) = opened {
		session.send(command);
	}
	let deadline = Instant::now() + WAIT;
	loop {
		pump(&session, &heard, &view, &timeline);
		app.frame();
		show(&gallery, &view);
		let tiles = view.visible_images().len();
		if tiles > GRID_IMAGES && view.settled() {
			break;
		}
		assert!(Instant::now() < deadline, "the channel's images did not arrive");
		std::thread::sleep(Duration::from_millis(2));
	}
	let filled = started.elapsed();
	let recording = server.control().recording();
	let attachments: Vec<_> = recording.media_to("/cdn/attachments/").collect();
	eprintln!(
		"image channel: on-screen images {:?} after opening, {} visible, {} attachment fetches",
		filled,
		view.visible_images().len(),
		attachments.len()
	);
	assert!(
		attachments.iter().all(|request| request.http2),
		"mock media goes over one multiplexed HTTP/2 connection as the CDN's does"
	);
}
