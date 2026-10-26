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

const LAYERS: [&str; 11] = [
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
];

const LUAU_BUTTONS: [(&str, &str); 2] = [
	("top_bar_threads", "threads.open_list"),
	("top_bar_pins", "threads.open_pins"),
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
		_ => unreachable!("no layer named {name}"),
	}
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
