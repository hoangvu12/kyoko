use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::{FrameCall, FrameRect, ProviderPlayer, RowKey, StandInFrames, Timeline};
use media::{Gallery, Limits, MediaService};
use mockcord::providers::{SPOTIFY_LINK, YOUTUBE_LINK};
use mockcord::{MockConfig, Server};
use protocol::fingerprint::Fingerprint;
use protocol::{Snowflake, Token};
use session::{Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Modifiers, PointerButton};
use zgx::scene::Rect;
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

fn click(app: &mut TestApp, id: &str) {
	let bounds = app.bounds(id);
	let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
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

fn device(rect: Rect) -> FrameRect {
	FrameRect {
		x: rect.x.round() as i32,
		y: rect.y.round() as i32,
		width: (rect.x + rect.width).round() as i32 - rect.x.round() as i32,
		height: (rect.y + rect.height).round() as i32 - rect.y.round() as i32,
	}
}

fn last_call(host: &StandInFrames) -> Option<FrameCall> {
	host.calls().last().cloned()
}

#[test]
fn play_opens_one_sandboxed_player_in_place_that_hides_under_overlays_and_dies_off_screen() {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let server = tokio
		.block_on(Server::start(MockConfig::default()))
		.expect("mockcord starts");
	let root = scratch("provider-embeds");
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
	let (into, shown) = (made.clone(), made.clone());
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
	let players = view.pictures.players();
	let host = StandInFrames::new(true);
	players.set_host(Box::new(host.clone()));
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

	let mut keys = Vec::new();
	for link in [YOUTUBE_LINK, SPOTIFY_LINK] {
		let message_id = server
			.post_message(channel_id, link)
			.expect("mockcord posts the link");
		let key = format!("embed-{message_id}-0");
		let play = format!("provider-play-{key}");
		assert!(
			app.wait_until(|app| {
				pump(&session, &heard, &view, &timeline);
				app.find(&play).is_some()
			}),
			"{link} shows its card with a play button"
		);
		keys.push(key);
	}
	let (youtube, spotify) = (keys[0].clone(), keys[1].clone());
	let row = |key: &str| {
		RowKey::Message(Snowflake(
			key.split('-')
				.nth(1)
				.and_then(|id| id.parse().ok())
				.expect("a message id"),
		))
	};
	assert!(view.scroll_to_end(&row(&spotify)));
	app.frames(10);
	assert!(
		host.calls().is_empty(),
		"no player and no autoplay before the click"
	);
	assert!(
		app.find(&format!("provider-open-{youtube}")).is_some(),
		"the card can open the link in the browser"
	);

	click(&mut app, &format!("provider-play-{youtube}"));
	let frame = format!("provider-frame-{youtube}");
	assert_eq!(players.live().as_deref(), Some(youtube.as_str()));
	assert!(
		app.find(&frame).is_some(),
		"the player takes the thumbnail's place"
	);
	let expected = timeline
		.borrow()
		.messages()
		.find(|entry| entry.message.content == YOUTUBE_LINK)
		.and_then(|entry| ProviderPlayer::of(&entry.message.embeds[0]))
		.expect("the YouTube embed's player");
	assert!(
		matches!(host.calls().first(), Some(FrameCall::Open { document, .. }) if *document == expected.document()),
		"the frame loads only kyoko's sandbox page around the embed URL: {:?}",
		host.calls().first()
	);
	assert!(
		app.wait_until(|_| host.placement().is_some()),
		"the player is placed over its box"
	);
	let bounds = app.bounds(&frame);
	assert_eq!(
		(bounds.width, bounds.height),
		expected.size,
		"Discord's 400 by 225"
	);
	assert_eq!(host.placement(), Some((device(bounds), device(bounds))));
	assert!(
		app.find(&format!("provider-close-{youtube}")).is_some(),
		"an open player can be closed"
	);

	let image = timeline
		.borrow()
		.messages()
		.find(|entry| entry.message.content == YOUTUBE_LINK)
		.and_then(|entry| bridge::LightboxImage::embed(entry.message.embeds[0].thumbnail.as_ref()?))
		.expect("the thumbnail as a lightbox image");
	view.show_viewer(bridge::Viewing::alone(image));
	assert!(
		app.wait_until(|_| last_call(&host) == Some(FrameCall::Hide { id: 1 })),
		"an overlay over the timeline hides the player: {:?}",
		last_call(&host)
	);
	assert!(view.close_lightbox());
	assert!(
		app.wait_until(|_| matches!(last_call(&host), Some(FrameCall::Place { id: 1, .. }))),
		"closing the overlay shows it again"
	);

	click(&mut app, &format!("provider-play-{spotify}"));
	assert_eq!(players.live().as_deref(), Some(spotify.as_str()));
	assert_eq!(host.open_frames(), 1, "one live player at a time");
	assert!(
		app.find(&frame).is_none() && app.find(&format!("provider-play-{youtube}")).is_some(),
		"the first card is back at rest"
	);
	assert!(
		app.wait_until(|_| host.placement().is_some()),
		"the second player is placed"
	);
	assert_eq!(
		app.bounds(&format!("provider-frame-{spotify}")).height,
		80.,
		"Spotify's track player is 80 high"
	);

	click(&mut app, &format!("provider-close-{spotify}"));
	assert_eq!(players.live(), None, "the close button destroys the player");
	assert_eq!(host.open_frames(), 0);

	click(&mut app, &format!("provider-play-{youtube}"));
	assert!(app.wait_until(|_| host.placement().is_some()));
	assert!(view.scroll_to(&RowKey::Start) || view.scroll_to(&RowKey::Loading));
	assert!(
		app.wait_until(|_| players.live().is_none()),
		"scrolling far away destroys the player: {:?}",
		last_call(&host)
	);
	assert_eq!(host.open_frames(), 0);

	let missing = StandInFrames::new(false);
	players.set_host(Box::new(missing.clone()));
	assert!(view.scroll_to_end(&row(&spotify)));
	assert!(
		app.wait_until(|app| app
			.texts()
			.iter()
			.any(|text| text.contains("WebView2 isn't installed"))),
		"without WebView2 the card says the link opens in the browser"
	);
	click(&mut app, &format!("provider-play-{youtube}"));
	assert_eq!(players.live(), None, "play falls back to the browser");
	assert!(missing.calls().is_empty());
}
