use std::cell::RefCell;
use std::rc::Rc;

use media::{Gallery, Limits, MediaFetcher, MediaService};
use serde_json::json;
use zgx::compose::prelude::*;
use zgx::reactive::Runtime;
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::expressions::Expressions;
use super::scripts::{WIDE, host_names, member_window, rail_items, shows, sidebar_rows};
use super::{Slot, layered, themes_dir, threads, timeline_slot, ui_dir};
use crate::community::CommunityLayer;
use crate::moderation::ModerationLayer;
use crate::profile::ProfileLayer;
use crate::switcher::SwitcherView;
use crate::theme::Catalog;
use crate::threads::ThreadsLayer;
use crate::timeline::pictures::Pictures;
use crate::timeline::{Prefs, Setup, TimelineView};
use crate::toasts::ToastLayer;

const LAYERS: [&str; 15] = [
	"timeline slot",
	"thread slot",
	"scrim",
	"profiles",
	"quick switcher",
	"thread popouts and dialogs",
	"community dialogs",
	"expression picker",
	"lightbox",
	"moderation dialogs and member menu",
	"toasts",
	"composer menu, attachment dialog and drop card",
	"friend confirmations",
	"apps launcher",
	"inbox popout",
];

const LUAU_BUTTONS: [(&str, &str); 3] = [
	("top_bar_threads", "threads.open_list"),
	("top_bar_pins", "threads.open_pins"),
	("top_bar_inbox", "inbox.open"),
];

#[test]
fn presses_reach_the_luau_tree_through_every_closed_layer() {
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let swallowed: Vec<String> = LAYERS
		.iter()
		.filter_map(|layer| {
			let missed = missed_presses(&tokio, &[layer]);
			(!missed.is_empty()).then(|| format!("{layer} swallowed {missed:?}"))
		})
		.collect();
	assert!(
		swallowed.is_empty(),
		"closed layers of the root overlay swallow presses meant for the Luau tree:\n{}",
		swallowed.join("\n")
	);
	let missed = missed_presses(&tokio, &LAYERS);
	assert!(
		missed.is_empty(),
		"the full root overlay swallows presses on {missed:?}"
	);
}

fn missed_presses(tokio: &tokio::runtime::Runtime, layers: &[&str]) -> Vec<&'static str> {
	let pressed: Rc<RefCell<Vec<String>>> = Rc::default();
	let built: Rc<RefCell<Vec<View>>> = Rc::default();
	let (seen, into) = (pressed.clone(), built.clone());
	let handle = tokio.handle().clone();
	let names: Vec<String> = layers.iter().map(ToString::to_string).collect();
	let mut app = TestApp::wrapped(
		move |runtime| {
			Catalog::load(&[themes_dir()]).install(runtime, "dark");
			let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
			for name in host_names() {
				host.action(name.clone(), |_| {});
				host.function(name, |_| Value::Nil);
			}
			for (_, action) in LUAU_BUTTONS {
				let seen = seen.clone();
				host.action(action, move |_| seen.borrow_mut().push(action.to_owned()));
			}
			let gallery = gallery(&handle);
			let pictures = Rc::new(Pictures::new(
				runtime.clone(),
				gallery.clone(),
				media::cdn::CDN.to_owned(),
				None,
			));
			into.borrow_mut()
				.extend(names.iter().map(|name| layer(name, runtime, &gallery, &pictures)));
			host
		},
		move |luau| layered(luau, built.borrow_mut().drain(..)).on_event(|_| {}),
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"a server channel",
		"Playing Chess",
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("sidebar.guild_id", json!("10")),
			("sidebar.guild_name", json!("Kyoko")),
			("sidebar.rows", sidebar_rows()),
			("timeline.channel_id", json!("20")),
			("timeline.channel_name", json!("general")),
			("composer.open", json!(true)),
			("members.heights", json!([44, 42, 42])),
			("members.window", member_window()),
		],
	);
	LUAU_BUTTONS
		.iter()
		.filter(|(button, action)| {
			app.click(button);
			app.frames(1);
			!pressed.borrow().iter().any(|seen| seen == action)
		})
		.map(|(button, _)| *button)
		.collect()
}

fn gallery(handle: &tokio::runtime::Handle) -> Gallery {
	let fetcher = MediaFetcher::new().expect("a media fetcher");
	let cache = std::env::temp_dir().join(format!("kyoko-root-test-{}", std::process::id()));
	Gallery::new(
		MediaService::new(fetcher, cache, Limits::default()),
		handle.clone(),
	)
}

fn layer(name: &str, runtime: &Runtime, gallery: &Gallery, pictures: &Rc<Pictures>) -> View {
	let pictures = pictures.clone();
	let timeline = || {
		TimelineView::new(
			runtime,
			Setup {
				gallery: gallery.clone(),
				cdn: media::cdn::CDN.to_owned(),
				mock_origin: None,
				code_font: "Consolas".to_owned(),
				prefs: Prefs::default(),
				store: None,
				developer_mode: false,
				pictures: Some(pictures.clone()),
				display: bridge::Display::default(),
			},
		)
	};
	match name {
		"timeline slot" => timeline_slot(
			runtime.signal(Slot::default()),
			timeline().view(),
			[runtime.signal(false), runtime.signal(false)],
		),
		"thread slot" => threads::thread_slot(
			runtime.signal(Slot::default()),
			runtime.signal(Slot::default()),
			timeline().view(),
		),
		"profiles" => ProfileLayer::new(runtime, pictures).view(),
		"quick switcher" => SwitcherView::new(runtime, pictures).view(),
		"thread popouts and dialogs" => ThreadsLayer::new(runtime, pictures).view(),
		"community dialogs" => CommunityLayer::new(runtime, pictures).view(),
		"expression picker" => Expressions::new(runtime, pictures).view(),
		"lightbox" => timeline().lightbox_view(),
		"moderation dialogs and member menu" => ModerationLayer::new(runtime).view(),
		"toasts" => ToastLayer::new(runtime).view(),
		"scrim" => crate::scrim::Scrim::new(runtime).view(),
		"composer menu, attachment dialog and drop card" => {
			crate::composer_layer::ComposerLayer::new(runtime, pictures).view()
		}
		"friend confirmations" => crate::friend_dialog::FriendDialog::new(runtime).view(),
		"apps launcher" => super::apps::Apps::new(runtime, pictures).view(),
		"inbox popout" => crate::inbox::InboxLayer::new(runtime, pictures).view(),
		_ => unreachable!("no layer named {name}"),
	}
}

type DialogParts = (Rc<ModerationLayer>, Rc<ToastLayer>);

fn on_top(app: &mut TestApp, id: &str) -> bool {
	let Some(node) = app.find(id) else {
		return false;
	};
	let bounds = app.bounds(id);
	let scene = app.ui.scene.borrow();
	scene
		.hit_test(bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.)
		.is_some_and(|hit| hit == node || scene.ancestors(hit).any(|above| above == node))
}

fn newest_toast(app: &mut TestApp) -> Option<String> {
	(0..32)
		.rev()
		.map(|id| format!("toast-{id}"))
		.find(|name| app.find(name).is_some())
}

fn settings_open_with(app: &mut TestApp, screen: &str, marker: &str, values: &[(&str, serde_json::Value)]) {
	shows(app, screen, marker, values);
	app.frames(30);
}

fn rust_overlays_show_over(
	app: &mut TestApp,
	parts: &DialogParts,
	requests: &Rc<RefCell<Vec<String>>>,
	dialog: &str,
) {
	use crate::moderation::{Confirm, ConfirmAction, MemberMenu};
	let (moderation, toasts) = parts;
	let (guild_id, user_id) = (protocol::Snowflake(10), protocol::Snowflake(30));
	moderation.confirm(Confirm {
		title: "Delete emoji?".to_owned(),
		body: ":pepe_ok: will be removed from this server.".to_owned(),
		label: "Delete",
		danger: true,
		action: ConfirmAction::DeleteEmoji {
			guild_id,
			emoji_id: protocol::Snowflake(33),
		},
	});
	app.frames(30);
	assert!(
		on_top(app, "moderation-confirm"),
		"a confirmation opened from {dialog} draws over it"
	);
	app.click("moderation-confirm");
	app.frames(2);
	assert!(
		requests
			.borrow()
			.iter()
			.any(|request| request.contains("DeleteEmoji")),
		"the confirmation over {dialog} takes the click: {:?}",
		requests.borrow()
	);
	moderation.kick(guild_id, user_id, "Brave Ember");
	app.frames(30);
	assert!(
		on_top(app, "moderation-dialog"),
		"the kick dialog draws over {dialog}"
	);
	app.key(zgx::input::Key::Escape);
	app.frames(30);
	assert!(
		!on_top(app, "moderation-dialog"),
		"Escape closes the kick dialog first"
	);
	assert!(
		on_top(app, &format!("{dialog}_dialog")),
		"{dialog} stays open under it"
	);
	moderation.open_menu(
		MemberMenu {
			guild_id,
			user_id,
			name: "Brave Ember".to_owned(),
			nick: String::new(),
			head: false,
			developer: false,
			items: vec![bridge::ModerationItem::Kick, bridge::ModerationItem::Ban],
		},
		(700., 300.),
	);
	app.frames(10);
	assert!(
		on_top(app, "moderation-member-menu"),
		"the member menu draws over {dialog}"
	);
	moderation.close_menu();
	app.frames(10);
	toasts.push(bridge::ToastKind::Error, "You are being rate limited.");
	app.frames(30);
	let toast = newest_toast(app).expect("a toast shows");
	assert!(
		on_top(app, &toast),
		"a toast shown while {dialog} is open draws over it"
	);
	toasts.push(bridge::ToastKind::Success, "Saved");
	app.frames(30);
	let toast = newest_toast(app).expect("a second toast shows");
	assert!(
		on_top(app, &toast),
		"a second toast shown over {dialog} draws over it too"
	);
}

#[test]
fn rust_dialogs_menus_and_toasts_open_above_the_settings_dialogs() {
	let parts: Rc<RefCell<Option<DialogParts>>> = Rc::default();
	let requests: Rc<RefCell<Vec<String>>> = Rc::default();
	let (made, used, heard) = (parts.clone(), parts.clone(), requests.clone());
	let mut app = TestApp::wrapped(
		move |runtime| {
			Catalog::load(&[themes_dir()]).install(runtime, "dark");
			let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
			for name in host_names() {
				host.action(name.clone(), |_| {});
				host.function(name, |_| Value::Nil);
			}
			let moderation = ModerationLayer::new(runtime);
			moderation.set_window(WIDE.0, WIDE.1);
			let heard = heard.clone();
			moderation.on_request(move |request| heard.borrow_mut().push(format!("{request:?}")));
			let toasts = ToastLayer::new(runtime);
			toasts.set_window(WIDE.0, WIDE.1);
			*made.borrow_mut() = Some((moderation, toasts));
			host
		},
		move |luau| {
			let (moderation, toasts) = used.borrow().clone().expect("the dialog parts");
			layered(luau, [moderation.view(), toasts.view()])
		},
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let parts = parts.borrow().clone().expect("the dialog parts");
	settings_open_with(
		&mut app,
		"server settings",
		"Server Ban List",
		&[
			("login.screen", json!("main")),
			("server.guild", super::scripts::server_guild()),
			(
				"server.bans",
				json!({ "loaded": true, "has_more": false, "rows": [] }),
			),
			("server.page", json!("bans")),
		],
	);
	rust_overlays_show_over(&mut app, &parts, &requests, "server");
	settings_open_with(
		&mut app,
		"closed server settings",
		"Find or start a conversation",
		&[("server.page", json!(""))],
	);
	settings_open_with(
		&mut app,
		"user settings",
		"Edit Profiles",
		&[("settings.page", json!("themes"))],
	);
	rust_overlays_show_over(&mut app, &parts, &requests, "settings");
}

const TOAST_SLOT: Slot = Slot {
	x: 400.,
	y: 80.,
	width: 600.,
	height: 500.,
};
const TOAST_ROWS: usize = 12;

type ToastParts = (
	Rc<ToastLayer>,
	zgx::reactive::Signal<Slot>,
	[zgx::reactive::Signal<bool>; 2],
);

#[test]
fn a_toast_floats_over_the_timeline_and_never_moves_its_rows() {
	let parts: Rc<RefCell<Option<ToastParts>>> = Rc::default();
	let (made, used) = (parts.clone(), parts.clone());
	let mut app = TestApp::wrapped(
		move |runtime| {
			Catalog::load(&[themes_dir()]).install(runtime, "dark");
			let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
			for name in host_names() {
				host.action(name.clone(), |_| {});
				host.function(name, |_| Value::Nil);
			}
			let layer = ToastLayer::new(runtime);
			layer.set_window(WIDE.0, WIDE.1);
			layer.set_area(Some(crate::toasts::Area {
				x: TOAST_SLOT.x,
				y: TOAST_SLOT.y,
				width: TOAST_SLOT.width,
				height: TOAST_SLOT.height,
			}));
			*made.borrow_mut() = Some((
				layer,
				runtime.signal(TOAST_SLOT),
				[runtime.signal(false), runtime.signal(false)],
			));
			host
		},
		move |luau| {
			let (layer, slot, hidden) = used.borrow().clone().expect("the toast parts");
			let rows = column().w_full().h_full().justify_end().children(
				(0..TOAST_ROWS).map(|index| div().id(format!("timeline-row-{index}")).w_full().h(40.)),
			);
			layered(luau, [timeline_slot(slot, rows, hidden), layer.view()])
		},
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	app.frames(2);
	let rows = |app: &mut TestApp| -> Vec<_> {
		(0..TOAST_ROWS)
			.map(|index| app.bounds(&format!("timeline-row-{index}")))
			.collect()
	};
	let before = rows(&mut app);
	let layer = parts.borrow().as_ref().expect("the toast parts").0.clone();
	layer.push(bridge::ToastKind::Error, "You are being rate limited.");
	layer.push(bridge::ToastKind::Success, "Link copied");
	app.frames(30);
	assert_eq!(
		before,
		rows(&mut app),
		"the timeline's rows moved while toasts showed"
	);
	let shown: Vec<_> = (0..4)
		.filter_map(|id| {
			let name = format!("toast-{id}");
			app.find(&name).map(|_| app.bounds(&name))
		})
		.collect();
	assert_eq!(shown.len(), 2, "both toasts show");
	let slot_bottom = TOAST_SLOT.y + TOAST_SLOT.height;
	for toast in &shown {
		assert!(
			toast.y >= TOAST_SLOT.y && toast.y + toast.height <= slot_bottom,
			"a toast floats over the timeline, inside its box: {toast:?}"
		);
		let center = toast.x + toast.width / 2.;
		assert!(
			(center - (TOAST_SLOT.x + TOAST_SLOT.width / 2.)).abs() < 1.,
			"a toast is centred on the timeline: {toast:?}"
		);
	}
}

type PollParts = (
	Rc<crate::composer_layer::ComposerLayer>,
	Rc<RefCell<Vec<crate::composer_layer::Request>>>,
);

fn sent_poll(requests: &RefCell<Vec<crate::composer_layer::Request>>) -> Option<session::PollDraft> {
	requests.borrow().iter().find_map(|request| match request {
		crate::composer_layer::Request::SendPoll(_, draft) => Some(draft.clone()),
		_ => None,
	})
}

#[test]
fn the_plus_menu_opens_the_poll_dialog_which_posts_a_draft_within_the_limits() {
	use crate::composer_layer::{ComposerLayer, Pane, PlusMenu, Request};
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let handle = tokio.handle().clone();
	let parts: Rc<RefCell<Option<PollParts>>> = Rc::default();
	let (made, used) = (parts.clone(), parts.clone());
	let mut app = TestApp::wrapped(
		move |runtime| {
			Catalog::load(&[themes_dir()]).install(runtime, "dark");
			let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
			for name in host_names() {
				host.action(name.clone(), |_| {});
				host.function(name, |_| Value::Nil);
			}
			let pictures = Rc::new(Pictures::new(
				runtime.clone(),
				gallery(&handle),
				media::cdn::CDN.to_owned(),
				None,
			));
			let layer = ComposerLayer::new(runtime, pictures);
			layer.set_window(WIDE.0, WIDE.1);
			let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
			let heard = requests.clone();
			layer.on_request(move |request| heard.borrow_mut().push(request));
			*made.borrow_mut() = Some((layer, requests));
			host
		},
		move |luau| {
			let (layer, _) = used.borrow().clone().expect("the composer layer");
			layered(luau, [layer.view()])
		},
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let (layer, requests) = parts.borrow().clone().expect("the composer layer");
	layer.open_menu(PlusMenu {
		pane: Pane::Main,
		at: (400., 700.),
		thread: false,
		poll: true,
		apps: true,
	});
	app.frames(10);
	app.click("composer-menu-poll");
	app.frames(30);
	assert!(on_top(&mut app, "poll-dialog"), "Create Poll opens the dialog");
	assert!(app.find("poll-answer-1").is_some() && app.find("poll-answer-2").is_none());
	app.click("poll-post");
	app.frames(2);
	assert!(sent_poll(&requests).is_none(), "an empty poll is not posted");
	assert!(
		app.opacity("poll-post") < 1.,
		"Post looks disabled while the poll is empty"
	);
	app.type_text("poll-question", "What should we play tonight?");
	app.type_text("poll-answer-0", "Valorant");
	app.type_text("poll-answer-1", &"a".repeat(60));
	app.click("poll-add-answer");
	app.frames(5);
	app.type_text("poll-answer-2", "Lethal Company");
	app.click("poll-add-answer");
	app.frames(5);
	assert!(
		app.find("poll-answer-3").is_some(),
		"Add another answer adds a field"
	);
	app.click("poll-remove-3");
	app.frames(5);
	assert!(
		app.find("poll-answer-3").is_none(),
		"the remove button drops a field"
	);
	app.click("poll-emoji-0");
	app.frames(2);
	assert!(
		requests.borrow().contains(&Request::PollEmoji),
		"an answer's emoji button asks for the emoji picker"
	);
	app.click("poll-multiple");
	app.frames(5);
	assert_eq!(
		app.opacity("poll-post"),
		1.,
		"Post is enabled once the poll is valid"
	);
	app.click("poll-post");
	app.frames(30);
	let draft = sent_poll(&requests).expect("the poll is posted");
	assert_eq!(draft.question, "What should we play tonight?");
	let answers: Vec<usize> = draft
		.answers
		.iter()
		.map(|answer| answer.text.chars().count())
		.collect();
	assert_eq!(
		answers,
		[8, session::POLL_ANSWER_MAX, 14],
		"answers stop at 55 characters"
	);
	assert_eq!(draft.duration_hours, session::POLL_DEFAULT_HOURS);
	assert!(draft.multiple);
	assert!(draft.problem().is_none());
	assert!(!on_top(&mut app, "poll-dialog"), "posting closes the dialog");
	layer.open_poll(Pane::Main);
	app.frames(30);
	assert!(on_top(&mut app, "poll-dialog"));
	assert!(
		app.find("poll-answer-2").is_none(),
		"the dialog opens empty again"
	);
	app.key(zgx::input::Key::Escape);
	app.frames(30);
	assert!(!on_top(&mut app, "poll-dialog"), "Escape closes the dialog");
}

type LauncherParts = (
	Rc<crate::apps_launcher::AppsLauncherView>,
	Rc<RefCell<Vec<crate::apps_launcher::Pick>>>,
);

fn launcher_model() -> bridge::AppsLauncher {
	let app = |id: u64, name: &str, hue: usize| bridge::LauncherApp {
		app: bridge::CommandApp {
			id: Some(protocol::Snowflake(id)),
			name: name.to_owned(),
			icon: None,
			hue,
		},
		description: format!("{name} does things"),
	};
	bridge::AppsLauncher {
		recents: vec![app(2, "Remindy", 1)],
		apps: (1..=8)
			.map(|id| app(id, &format!("App {id}"), id as usize))
			.collect(),
	}
}

#[test]
fn the_apps_button_opens_the_launcher_and_escape_or_a_press_outside_closes_it() {
	use crate::apps_launcher::{AppsLauncherView, Pick};
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let handle = tokio.handle().clone();
	let parts: Rc<RefCell<Option<LauncherParts>>> = Rc::default();
	let (made, used) = (parts.clone(), parts.clone());
	let mut app = TestApp::wrapped(
		move |runtime| {
			Catalog::load(&[themes_dir()]).install(runtime, "dark");
			let host = ScriptHost::new(runtime, ui_dir().join("app.luau"));
			for name in host_names() {
				host.action(name.clone(), |_| {});
				host.function(name, |_| Value::Nil);
			}
			let pictures = Rc::new(Pictures::new(
				runtime.clone(),
				gallery(&handle),
				media::cdn::CDN.to_owned(),
				None,
			));
			let launcher = AppsLauncherView::new(runtime, pictures);
			let picks: Rc<RefCell<Vec<Pick>>> = Rc::default();
			let heard = picks.clone();
			launcher.on_pick(move |pick| heard.borrow_mut().push(pick));
			let opened = launcher.clone();
			host.action("composer.picker", move |argument| {
				let tab = super::field(&argument, "tab").and_then(super::text);
				if tab == Some(super::apps::APPS_TAB) {
					opened.open((900., 800.), 640., launcher_model(), true);
				}
			});
			*made.borrow_mut() = Some((launcher, picks));
			host
		},
		move |luau| {
			let (launcher, _) = used.borrow().clone().expect("the launcher");
			layered(luau, [launcher.view()])
		},
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(
		&mut app,
		"a server channel",
		"Playing Chess",
		&[
			("login.screen", json!("main")),
			("rail.items", rail_items()),
			("sidebar.guild_id", json!("10")),
			("sidebar.guild_name", json!("Kyoko")),
			("sidebar.rows", sidebar_rows()),
			("timeline.channel_id", json!("20")),
			("timeline.channel_name", json!("general")),
			("composer.open", json!(true)),
			("members.heights", json!([44, 42, 42])),
			("members.window", member_window()),
		],
	);
	let (launcher, picks) = parts.borrow().clone().expect("the launcher");
	let open = |app: &mut TestApp| {
		app.click("composer_apps_button");
		app.frames(30);
	};
	open(&mut app);
	assert!(launcher.is_open(), "the apps button opens the launcher");
	assert!(
		on_top(&mut app, "apps-app-1"),
		"the launcher lists the server's apps"
	);
	assert!(app.find("apps-recent-2").is_some(), "and the recent ones");
	let (list, frame) = (app.bounds("apps-list"), app.bounds("apps-launcher"));
	assert!(
		list.y + list.height <= frame.y + frame.height,
		"the app list stays inside the launcher: {list:?} in {frame:?}"
	);
	assert!(
		list.height > 6. * 56.,
		"the app list takes the room the launcher has: {list:?}"
	);
	app.key(zgx::input::Key::Escape);
	app.frames(30);
	assert!(!launcher.is_open(), "Escape closes the launcher");
	open(&mut app);
	assert!(launcher.is_open());
	app.click("top_bar_threads");
	app.frames(30);
	assert!(!launcher.is_open(), "a press outside closes the launcher");
	open(&mut app);
	app.click("apps-app-1");
	app.frames(30);
	assert!(!launcher.is_open(), "picking an app closes the launcher");
	assert_eq!(
		*picks.borrow(),
		[Pick::App(protocol::Snowflake(1))],
		"picking an app hands it to the composer's command picker"
	);
}
