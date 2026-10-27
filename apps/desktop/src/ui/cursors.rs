use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde_json::json;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::scene::NodeKind;
use zgx::semantics::Role;
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::root::{LAYERS, gallery, layer};
use super::scripts::{WIDE, host_names, member_window, rail_items, shows, sidebar_rows};
use super::{layered, themes_dir, ui_dir};
use crate::caret::Blink;
use crate::theme::Catalog;
use crate::timeline::pictures::Pictures;

const HALF: Duration = Duration::from_millis(500);
const COMPOSER_FIELD: &str = "composer_field";

#[test]
fn controls_show_the_hand_fields_the_i_beam_and_no_closed_layer_hides_either() {
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let mut app = shell_under_every_layer(tokio.handle());
	let mut rows = sidebar_rows();
	rows.as_array_mut().expect("sidebar rows").push(json!({
		"kind": "channel", "key": "h22", "id": "22", "type": "text", "icon": "hash", "name": "staff",
		"unread": false, "mentions": 0, "muted": false, "locked": true, "joined": false,
		"selectable": false, "thread": false, "last_thread": false, "voice": [],
	}));
	shows(
		&mut app,
		"a server channel",
		"Playing Chess",
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("sidebar.guild_id", json!("10")),
			("sidebar.guild_name", json!("Kyoko")),
			("sidebar.rows", rows),
			("timeline.channel_id", json!("20")),
			("timeline.channel_name", json!("general")),
			("composer.open", json!(true)),
			("members.heights", json!([44, 42, 42])),
			("members.window", member_window()),
		],
	);
	for id in ["top_bar_threads", "rail_home", "channel_20", "window_close"] {
		assert_eq!(cursor_over(&mut app, id), Cursor::Pointer, "over {id}");
	}
	assert_eq!(cursor_over(&mut app, COMPOSER_FIELD), Cursor::Text);
	assert_eq!(cursor_over(&mut app, "channel_22"), Cursor::NotAllowed);
	let masked = masked_pointers(&mut app);
	assert!(
		masked.is_empty(),
		"something above these controls hides their hand cursor: {masked:?}"
	);
}

#[test]
fn the_caret_blinks_holds_while_typing_and_stays_hidden_while_the_window_is_away() {
	let mut app = TestApp::new(|runtime| {
		Catalog::load(&[themes_dir()]).install(runtime, "midnight");
		stubbed_host(runtime)
	});
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"a server channel",
		"general",
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("sidebar.guild_id", json!("10")),
			("sidebar.guild_name", json!("Kyoko")),
			("sidebar.rows", sidebar_rows()),
			("timeline.channel_id", json!("20")),
			("timeline.channel_name", json!("general")),
			("composer.open", json!(true)),
		],
	);
	app.click(COMPOSER_FIELD);
	app.ui.prepare_frame();
	let blink = Blink::new(HALF);
	let start = Instant::now();
	let phases: Vec<bool> = (0..4)
		.map(|phase| {
			blink.tick(&app.ui, start + HALF * phase);
			caret_shows(&app)
		})
		.collect();
	assert_eq!(
		phases,
		[true, false, true, false],
		"the caret blinks at the given rate"
	);

	let typed = start + HALF * 3 + HALF / 2;
	app.ui.dispatch(InputEvent::Text("hello".into()));
	app.settle();
	app.ui.prepare_frame();
	assert!(caret_shows(&app), "typing shows the caret at once");
	let after_typing: Vec<bool> = [Duration::ZERO, HALF - Duration::from_millis(1), HALF, HALF * 2]
		.into_iter()
		.map(|since| {
			blink.tick(&app.ui, typed + since);
			caret_shows(&app)
		})
		.collect();
	assert_eq!(
		after_typing,
		[true, true, false, true],
		"the caret stays solid for a whole phase after a key, then blinks again"
	);

	let field = app.ui.focused_editor().expect("the composer keeps focus").node;
	app.ui.input.dispatch(&app.ui.scene, InputEvent::Blur);
	app.settle();
	app.ui.prepare_frame();
	let away = typed + HALF * 3;
	let while_away: Vec<bool> = (0..4)
		.map(|phase| {
			blink.tick(&app.ui, away + HALF * phase);
			caret_shows(&app)
		})
		.collect();
	assert_eq!(
		while_away, [false; 4],
		"no caret while the window is in the background"
	);

	app.ui.input.dispatch_to(&app.ui.scene, field, InputEvent::Focus);
	app.settle();
	app.ui.prepare_frame();
	let back = away + HALF * 4;
	let returned: Vec<bool> = (0..3)
		.map(|phase| {
			blink.tick(&app.ui, back + HALF * phase);
			caret_shows(&app)
		})
		.collect();
	assert_eq!(returned, [true, false, true], "blinking resumes, solid first");
}

fn stubbed_host(runtime: &zgx::reactive::Runtime) -> ScriptHost {
	let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
	for name in host_names() {
		host.action(name.clone(), |_| {});
		host.function(name, |_| Value::Nil);
	}
	host
}

fn shell_under_every_layer(handle: &tokio::runtime::Handle) -> TestApp {
	let built: Rc<RefCell<Vec<View>>> = Rc::default();
	let into = built.clone();
	let handle = handle.clone();
	let app = TestApp::wrapped(
		move |runtime| {
			Catalog::load(&[themes_dir()]).install(runtime, "midnight");
			let host = stubbed_host(runtime);
			let gallery = gallery(&handle);
			let pictures = Rc::new(Pictures::new(
				runtime.clone(),
				gallery.clone(),
				media::cdn::CDN.to_owned(),
				None,
			));
			into.borrow_mut().extend(
				LAYERS
					.iter()
					.map(|name| layer(name, runtime, &gallery, &pictures)),
			);
			host
		},
		move |luau| layered(luau, built.borrow_mut().drain(..)).on_event(|_| {}),
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	app
}

fn cursor_over(app: &mut TestApp, id: &str) -> Cursor {
	let bounds = app.bounds(id);
	let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
	app.ui.dispatch(InputEvent::PointerMove { x, y });
	app.settle();
	app.ui.prepare_frame();
	shown_cursor(&app.ui, x, y)
}

fn shown_cursor(ui: &zgx::widgets::Ui, x: f32, y: f32) -> Cursor {
	let scene = ui.scene.borrow();
	let pointer = ui
		.input
		.captured()
		.or(ui.input.hovered())
		.filter(|id| ui.input.is_enabled(&scene, *id));
	let role = pointer.and_then(|id| ui.semantics.borrow().get(id).map(|node| node.role));
	scene.cursor_at(x, y).unwrap_or(match role {
		Some(Role::Button | Role::Link | Role::CheckBox | Role::MenuItem) => Cursor::Pointer,
		Some(Role::TextInput | Role::MultilineTextInput) => Cursor::Text,
		Some(Role::Slider) => Cursor::EwResize,
		_ => Cursor::Default,
	})
}

fn masked_pointers(app: &mut TestApp) -> Vec<String> {
	app.ui.prepare_frame();
	let ui = &app.ui;
	let scene = ui.scene.borrow();
	let mut masked = Vec::new();
	let mut stack = vec![scene.root()];
	while let Some(node) = stack.pop() {
		stack.extend(scene.children(node).iter().copied());
		if !ui.input.has_listeners(node)
			|| !ui.input.is_enabled(&scene, node)
			|| scene.cursor_for(node) != Some(Cursor::Pointer)
		{
			continue;
		}
		let bounds = scene.bounds(node);
		let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
		let hits = scene.hit_test_all(x, y);
		let Some(&top) = hits.first() else {
			continue;
		};
		if hits.contains(&node) && top != node && !scene.ancestors(top).any(|above| above == node) {
			masked.push(format!("{node:?} at {x},{y} under {top:?}"));
		}
	}
	masked
}

fn caret_shows(app: &TestApp) -> bool {
	let editor = app.ui.focused_editor().expect("a focused field");
	let scene = app.ui.scene.borrow();
	scene.effects(editor.caret).opacity > 0.
		&& !matches!(scene.kind(editor.caret), NodeKind::Rect(color) if color.3 == 0)
}
