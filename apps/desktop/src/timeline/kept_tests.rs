use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use bridge::Row;
use media::{Gallery, Limits, MediaFetcher, MediaService};
use plugins::{HostConfig, Permission, PluginHost};
use protocol::{Message, Snowflake};
use serde_json::json;
use session::{DisplayHooks, HistoryLook, KeptMessage, KeptVersion};
use state::State;
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{Prefs, Setup, TimelineView, rows};
use crate::plugin_menu::{AccessoryClick, PluginMenu};

const LOGGER: &str = "message-logger";
const EARLIER: &str = "See you at 6";
const CURRENT: &str = "See you at 7";
const HISTORY: &str = "kept-version-100-0";
const REMOVE: &str = "accessory-message-logger-100";
const STEP: f32 = 6.;

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn logger(root: &std::path::Path) -> PluginHost {
	let host = PluginHost::new(HostConfig {
		data_dir: Some(root.join("plugins")),
		user_dir: None,
		development_dirs: Vec::new(),
		bundled: true,
		limits: plugins::Limits {
			load_deadline: Duration::from_secs(15),
			..plugins::Limits::default()
		},
	});
	let declared: Vec<Permission> = host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == LOGGER)
		.map(|plugin| plugin.permissions)
		.expect("MessageLogger is bundled");
	host.approve(LOGGER, &declared).expect("MessageLogger starts");
	host.set_setting(LOGGER, "inline_edits", &json!(false))
		.expect("a setting");
	host
}

fn kept() -> KeptMessage {
	KeptMessage {
		id: Snowflake(100),
		channel_id: Snowflake(7),
		guild_id: None,
		author_id: Snowflake(300),
		bot: false,
		mine: false,
		content: CURRENT.to_owned(),
		deleted_at: None,
		edited_at: Some(1_790_000_100),
		versions: vec![KeptVersion {
			content: EARLIER.to_owned(),
			edited_at: None,
			replaced_at: Some(1_790_000_100),
			attachments: 0,
			embeds: 0,
		}],
	}
}

fn click_at(app: &mut TestApp, x: f32, y: f32) {
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
	app.settle();
	app.frames(2);
}

#[test]
fn a_collapsed_history_opens_from_edited_and_remove_from_log_reaches_the_plugin() {
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let handle = tokio.handle().clone();
	let root = scratch("kept-row");
	let host = logger(&root);
	let look = host
		.render_message(&kept())
		.expect("MessageLogger decorates the edit");
	assert_eq!(look.history, Some(HistoryLook::Collapsed));
	let message: Message = serde_json::from_value(json!({
		"id": "100",
		"channel_id": "7",
		"author": { "id": "300", "username": "ada" },
		"content": CURRENT,
		"timestamp": "2026-10-01T12:00:00+00:00",
		"edited_timestamp": "2026-10-01T12:05:00+00:00",
	}))
	.expect("the message decodes");
	let mut entry = bridge::detached_entry(&State::new(), None, &Arc::new(message));
	entry.look = Some(look);
	entry.history = Arc::from([markdown::parse(EARLIER)]);
	let row = Rc::new(Row::Message(Box::new(entry)));

	let clicks: Rc<RefCell<Vec<AccessoryClick>>> = Rc::default();
	let heard = clicks.clone();
	let gesture_host = host.clone();
	let built: Rc<RefCell<Option<View>>> = Rc::default();
	let into = built.clone();
	let mut app = TestApp::wrapped(
		move |runtime| {
			let fetcher = MediaFetcher::new().expect("a media fetcher");
			let service = MediaService::new(fetcher, scratch("kept-row-media"), Limits::default());
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
			let menu = PluginMenu::new();
			menu.on_accessory(move |click| {
				gesture_host
					.gesture(
						&click.plugin,
						&click.key,
						&[("message_id".to_owned(), click.message_id.to_string())],
					)
					.expect("the plugin hears the click");
				heard.borrow_mut().push(click);
			});
			view.set_plugin_menu(menu);
			let (env, hovered, bump) = (view.env.clone(), runtime.signal(false), runtime.signal(0u32));
			let watched = bump.clone();
			*into.borrow_mut() = Some(div().w(640.).child(switch(
				move || watched.get(),
				move |_: u32, _| rows::render(&row, &env, &hovered, &bump),
			)));
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| built.borrow_mut().take().unwrap_or_else(div),
	);
	assert!(
		app.wait_until(|app| app.find(REMOVE).is_some() && app.bounds("message-text-100").width > 0.),
		"the row and its Remove from log link are laid out"
	);
	assert!(app.find(HISTORY).is_none(), "a collapsed history starts closed");

	let text = app.bounds("message-text-100");
	let y = text.y + text.height / 2.;
	let mut x = text.x + 1.;
	while app.find(HISTORY).is_none() && x < text.x + text.width {
		click_at(&mut app, x, y);
		x += STEP;
	}
	assert!(app.find(HISTORY).is_some(), "clicking (edited) opens the history");
	let opened_at = x - STEP;
	let text = app.bounds("message-text-100");
	click_at(&mut app, opened_at, text.y + text.height / 2.);
	assert!(app.find(HISTORY).is_none(), "clicking (edited) again closes it");

	app.click(REMOVE);
	let clicked = clicks.borrow().clone();
	assert_eq!(
		clicked,
		vec![AccessoryClick {
			plugin: LOGGER.to_owned(),
			key: "remove".to_owned(),
			channel_id: Snowflake(7),
			message_id: Snowflake(100),
		}]
	);
	assert_eq!(
		host.render_message(&kept()),
		None,
		"MessageLogger stops decorating a removed message"
	);
	let _ = std::fs::remove_dir_all(&root);
}
