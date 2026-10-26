use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use media::{Gallery, ImageName, Limits, MediaFetcher, MediaService, Status};
use mockcord::{MockConfig, Server};
use protocol::fingerprint::Fingerprint;
use protocol::{Message, Snowflake};
use serde_json::json;
use zgx::compose::prelude::*;
use zgx::reactive::Runtime;
use zgx::scene::{NodeId, NodeKind, Scene};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::body::{self, Look};
use super::pictures::Pictures;
use super::{Prefs, Setup, TimelineView};

const CONTENT: &str = "ship it \u{1F680} <:party:11> and <a:dance:12> \u{1F44B}\u{1F3FD}!";
const WAIT: Duration = Duration::from_secs(15);

fn scratch(name: &str) -> std::path::PathBuf {
	std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()))
}

fn setup(gallery: Gallery, cdn: String, mock_origin: Option<String>) -> Setup {
	Setup {
		gallery,
		cdn,
		mock_origin,
		code_font: "Consolas".to_owned(),
		prefs: Prefs::default(),
		store: None,
		developer_mode: false,
		pictures: None,
		display: bridge::Display::default(),
	}
}

fn svgs(scene: &Scene, from: NodeId) -> usize {
	let own = usize::from(matches!(scene.kind(from), NodeKind::Svg(_)));
	own + scene
		.children(from)
		.iter()
		.map(|child| svgs(scene, *child))
		.sum::<usize>()
}

#[test]
fn copying_a_message_yields_its_unicode_emoji_and_custom_emoji_markup() {
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let handle = tokio.handle().clone();
	let built: Rc<RefCell<Option<View>>> = Rc::default();
	let into = built.clone();
	let mut app = TestApp::wrapped(
		move |runtime| {
			let fetcher = MediaFetcher::new().expect("a media fetcher");
			let service = MediaService::new(fetcher, scratch("emoji-copy"), Limits::default());
			let gallery = Gallery::new(service, handle);
			let view = TimelineView::new(runtime, setup(gallery, media::cdn::CDN.to_owned(), None));
			let document = markdown::parse(CONTENT);
			let bump = runtime.signal(0);
			let names = bridge::Names::default();
			let rendered = body::render(
				&document,
				&view.env,
				&names,
				Snowflake(1),
				0,
				&bump,
				Look::message(16.),
			);
			*into.borrow_mut() = Some(div().id("message").w(640.).selectable(true).child(rendered));
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| built.borrow_mut().take().unwrap_or_else(div),
	);
	app.frames(2);
	let message = app.find("message").expect("the message is mounted");
	assert_eq!(
		svgs(&app.ui.scene.borrow(), message),
		2,
		"the rocket and the waving hand are drawn from Twemoji"
	);
	assert!(app.ui.select_all_text(message), "the message text is selectable");
	assert_eq!(
		app.ui.selected_text(),
		"ship it \u{1F680} <:party:11> and <a:dance:12> \u{1F44B}\u{1F3FD}!"
	);
	let html = app.ui.selected_html();
	for part in ["\u{1F680}", "party:11", "dance:12", "\u{1F44B}\u{1F3FD}"] {
		assert!(html.contains(part), "{part} is missing from {html}");
	}
}

#[test]
fn unicode_emoji_never_reach_the_network() {
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
	let fingerprint = Fingerprint::web_client("en-US", 1);
	let fetcher =
		bridge::media_fetcher(&fingerprint, Some(server.address().port())).expect("a media fetcher");
	let service = MediaService::new(fetcher, scratch("emoji-network"), Limits::default());
	let gallery = Gallery::new(service, tokio.handle().clone());
	let origin = format!("http://{}", server.address());
	let pictures = Pictures::new(Runtime::new(), gallery.clone(), server.cdn_url(), Some(origin));
	let message: Message = serde_json::from_value(json!({
		"id": "100",
		"channel_id": "200",
		"author": { "id": "300", "username": "ada", "avatar": "abc" },
		"content": CONTENT,
		"timestamp": "2026-10-01T12:00:00+00:00",
		"reactions": [
			{ "emoji": { "id": null, "name": "\u{1F44D}" }, "count": 2, "me": false },
			{ "emoji": { "id": "13", "name": "blob" }, "count": 1, "me": true },
		],
	}))
	.expect("the message decodes");
	let entry = bridge::detached_entry(&state::State::new(), None, &Arc::new(message));
	let names = pictures.names_in(&entry, true);
	gallery.set_visible(names.iter().map(ImageName::as_str));
	let deadline = Instant::now() + WAIT;
	while !names
		.iter()
		.all(|name| matches!(gallery.status(name), Some(Status::Ready { .. } | Status::Failed)))
	{
		assert!(Instant::now() < deadline, "the images did not settle");
		std::thread::sleep(Duration::from_millis(20));
	}
	let recording = server.control().recording();
	let paths: Vec<&str> = recording
		.media_to("")
		.map(|request| request.path.as_str())
		.collect();
	for id in ["11", "12", "13"] {
		assert!(
			paths.iter().any(|path| path.contains(&format!("/emojis/{id}."))),
			"custom emoji {id} was fetched: {paths:?}"
		);
	}
	assert!(
		paths
			.iter()
			.all(|path| path.contains("/emojis/") || path.contains("/avatars/")),
		"only custom emoji and the avatar were fetched: {paths:?}"
	);
	assert_eq!(paths.len(), names.len(), "{paths:?} for {names:?}");
}
