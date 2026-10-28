use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::{RowKey, Timeline, VideoClip, VideoPhase, VideoState};
use media::{Gallery, Limits, MediaService};
use mockcord::{MockConfig, Server, Signature};
use protocol::fingerprint::Fingerprint;
use protocol::{Snowflake, Token};
use session::{Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Modifiers, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{Prefs, Setup, TimelineView};

const WAIT: Duration = Duration::from_secs(30);
const WINDOW: (f32, f32) = (1280., 800.);
const SLOT: (f32, f32) = (760., 700.);

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

fn middle(app: &mut TestApp, id: &str) -> (f32, f32) {
	let bounds = app.bounds(id);
	(bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.)
}

fn at_fraction(app: &mut TestApp, id: &str, fraction: f32) -> (f32, f32) {
	let bounds = app.bounds(id);
	(bounds.x + bounds.width * fraction, bounds.y + bounds.height / 2.)
}

fn hover(app: &mut TestApp, (x, y): (f32, f32)) {
	app.ui
		.dispatch_with_modifiers(InputEvent::PointerMove { x, y }, Modifiers::default());
	app.frames(4);
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

#[test]
fn a_video_attachment_plays_inline_with_discords_controls_and_pauses_off_screen() {
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
	let root = scratch("video-inline");
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
	let fingerprint = Fingerprint::web_client("en-US", 1);
	let fetcher =
		bridge::media_fetcher(&fingerprint, Some(server.address().port())).expect("a media fetcher");
	let gallery = Gallery::new(
		MediaService::new(fetcher, root.join("cache"), Limits::default()),
		tokio.handle().clone(),
	);
	let made: Rc<RefCell<Option<Rc<TimelineView>>>> = Rc::default();
	let screens: Rc<RefCell<Vec<bool>>> = Rc::default();
	let (into, shown, recorded) = (made.clone(), made.clone(), screens.clone());
	let (origin, cdn) = (format!("http://{}", server.address()), server.cdn_url());
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
			view.resize(SLOT.0, SLOT.1);
			view.set_window(WINDOW.0, WINDOW.1);
			view.on_fullscreen(move |on| recorded.borrow_mut().push(on));
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
						.child(view.video_view()),
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
		.post_video(channel_id, Signature::Fresh)
		.expect("mockcord posts the video");
	let posted = || {
		timeline
			.borrow()
			.messages()
			.find(|entry| entry.id == message_id)
			.and_then(|entry| VideoClip::attachment(&entry.message, &entry.message.attachments[0]))
	};
	assert!(
		app.wait_until(|app| {
			pump(&session, &heard, &view, &timeline);
			posted().is_some_and(|clip| app.find(&format!("video-{}", clip.key)).is_some())
		}),
		"the video attachment is laid out"
	);
	let clip = posted().expect("the posted clip");
	let watching = view.pictures.watching();
	let state = || -> VideoState { watching.player().state(&clip) };
	assert_eq!(state().phase, VideoPhase::Idle, "nothing plays before a click");
	let surface = format!("video-{}", clip.key);
	app.frames(10);

	let play = middle(&mut app, &format!("video-play-{}", clip.key));
	click(&mut app, play);
	assert!(
		app.wait_until(|_| watching.frames_shown() > 0 && state().phase == VideoPhase::Playing),
		"clicking play shows frames inline: {:?}",
		state()
	);
	let requests: Vec<_> = server
		.control()
		.recording()
		.media_to("/cdn/attachments/")
		.filter(|request| {
			request.path.ends_with(mockcord::clip::CLIP_FILENAME) && request.param("format").is_none()
		})
		.cloned()
		.collect();
	assert!(
		requests
			.iter()
			.all(|request| request.header("sec-fetch-dest") == Some("video")
				&& request
					.header("range")
					.is_some_and(|range| range.starts_with("bytes="))),
		"the video goes through the media client in ranges: {requests:?}"
	);

	let inside = middle(&mut app, &surface);
	hover(&mut app, inside);
	let controls = format!("video-controls-{}", clip.key);
	assert!(app.find(&controls).is_some(), "hovering shows the controls");
	let toggle = middle(&mut app, &format!("video-toggle-{}", clip.key));
	click(&mut app, toggle);
	assert!(
		app.wait_until(|_| state().phase == VideoPhase::Paused),
		"the play button pauses"
	);
	assert!(
		app.texts().iter().any(|text| text.ends_with("/ 0:03")),
		"the time shows the length"
	);

	let half = at_fraction(&mut app, &format!("video-seek-{}", clip.key), 0.5);
	click(&mut app, half);
	assert!(
		app.wait_until(
			|_| state().position.abs_diff(Duration::from_millis(1500)) <= Duration::from_millis(200)
		),
		"the seek bar seeks: {:?}",
		state()
	);

	let mute = middle(&mut app, &format!("video-mute-{}", clip.key));
	click(&mut app, mute);
	assert!(state().muted, "the volume button mutes");
	click(&mut app, mute);
	assert!(!state().muted, "and unmutes");

	let fullscreen = middle(&mut app, &format!("video-fullscreen-button-{}", clip.key));
	click(&mut app, fullscreen);
	assert!(
		watching.is_fullscreen(),
		"the fullscreen button opens the fullscreen player"
	);
	assert!(
		app.find("video-fullscreen").is_some(),
		"the fullscreen layer draws"
	);
	assert_eq!(*screens.borrow(), [true], "the window goes fullscreen");
	assert!(view.leave_video_fullscreen(), "Escape leaves fullscreen");
	app.frames(4);
	assert!(app.find("video-fullscreen").is_none());
	assert_eq!(*screens.borrow(), [true, false], "the window leaves fullscreen");

	let inside = middle(&mut app, &surface);
	click(&mut app, inside);
	assert!(
		app.wait_until(|_| state().phase == VideoPhase::Playing),
		"clicking the video plays it again"
	);
	assert!(view.scroll_to(&RowKey::Start) || view.scroll_to(&RowKey::Loading));
	assert!(
		app.wait_until(|_| state().phase == VideoPhase::Paused),
		"scrolling the video away pauses it: {:?}",
		state()
	);
}
