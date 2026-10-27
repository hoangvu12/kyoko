use std::cell::RefCell;
use std::rc::Rc;

use protocol::Snowflake;
use serde_json::json;
use state::{Event, Loaded, LocalEvent, State};
use zgx_script::testing::TestApp;
use zgx_script::{ScriptHost, Value};

use super::root::gallery;
use super::scripts::{WIDE, host_names};
use super::{layered, themes_dir, ui_dir};
use crate::profile::{Anchor, Opening, ProfileLayer, Request};
use crate::theme::Catalog;
use crate::timeline::pictures::Pictures;

const WING: Snowflake = Snowflake(4242);

type Parts = (Rc<ProfileLayer>, Rc<RefCell<Vec<String>>>);

fn wing_state() -> State {
	let mut state = State::new();
	let raw = json!({
		"user": { "id": WING.to_string(), "username": "zx0002", "global_name": "wing" },
		"user_profile": { "bio": "working on zeron", "theme_colors": [1_118_481, 6_684_927] },
		"mutual_guilds": [],
		"connected_accounts": [],
	});
	state.apply(&Event::Local(LocalEvent::Loaded(Box::new(Loaded::Profile {
		user_id: WING,
		guild_id: None,
		raw,
	}))));
	state
}

fn profile_app(tokio: &tokio::runtime::Runtime) -> (TestApp, Parts) {
	let handle = tokio.handle().clone();
	let parts: Rc<RefCell<Option<Parts>>> = Rc::default();
	let (made, used) = (parts.clone(), parts.clone());
	let app = TestApp::wrapped(
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
			let layer = ProfileLayer::new(runtime, pictures);
			layer.set_window(WIDE.0, WIDE.1);
			let requests: Rc<RefCell<Vec<String>>> = Rc::default();
			let heard = requests.clone();
			layer.on_request(move |request: Request| heard.borrow_mut().push(format!("{request:?}")));
			*made.borrow_mut() = Some((layer, requests));
			host
		},
		move |luau| {
			let (layer, _) = used.borrow().clone().expect("the profile layer");
			layered(luau, [layer.view()])
		},
	);
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	let parts = parts.borrow().clone().expect("the profile layer");
	(app, parts)
}

fn card_of(state: &State, full: bool) -> Option<(bridge::ProfileCard, Vec<Vec<bridge::ProfileMenuItem>>)> {
	let card = bridge::ProfileCard::of(state, WING, None, chrono::Utc::now())?;
	let menu = bridge::profile_menu(state, &card, full);
	Some((card, menu))
}

fn open_wing(app: &mut TestApp, layer: &ProfileLayer) {
	layer.open_popout(Opening {
		user_id: WING,
		guild_id: None,
		at: (400., 120.),
		anchor: Anchor::TopLeft,
	});
	layer.show_card(card_of(&wing_state(), false));
	app.frames(10);
}

fn tokio() -> tokio::runtime::Runtime {
	tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime")
}

#[test]
fn the_popout_more_button_opens_discords_menu_and_the_field_sends_a_message() {
	let tokio = tokio();
	let (mut app, (layer, requests)) = profile_app(&tokio);
	open_wing(&mut app, &layer);
	assert!(app.find("profile-popout").is_some(), "the popout opens");
	assert!(app.find("profile-effect").is_none(), "no effect without one");
	app.click("profile-popout-more");
	app.frames(10);
	for item in ["full-profile", "share", "ignore", "block", "report", "copy-id"] {
		assert!(
			app.find(&format!("profile-menu-{item}")).is_some(),
			"More shows {item}"
		);
	}
	app.click("profile-menu-ignore");
	app.frames(5);
	assert!(
		requests
			.borrow()
			.iter()
			.any(|request| request.contains("IgnoreUser")),
		"Ignore asks the session: {:?}",
		requests.borrow()
	);
	app.click("profile-popout-more");
	app.frames(10);
	app.click("profile-menu-report");
	app.frames(5);
	assert!(requests.borrow().iter().any(|request| request == "Report"));
	app.type_text("profile-popout-message-input", "hey wing");
	app.key(zgx::input::Key::Enter);
	app.frames(5);
	assert!(
		requests
			.borrow()
			.iter()
			.any(|request| request.contains("QuickMessage") && request.contains("hey wing")),
		"Enter sends the message: {:?}",
		requests.borrow()
	);
	assert!(app.find("profile-popout").is_none(), "sending closes the popout");
}

#[test]
fn the_full_profile_has_its_three_tabs_and_closes_from_its_button() {
	let tokio = tokio();
	let (mut app, (layer, _requests)) = profile_app(&tokio);
	layer.open_full(WING, None);
	layer.show_card(card_of(&wing_state(), true));
	app.frames(30);
	assert!(app.find("full-profile").is_some(), "the full profile opens");
	assert!(app.find("full-profile-card").is_some());
	assert_eq!(app.text("full-profile-tab-activity"), "Activity");
	assert_eq!(app.text("full-profile-tab-friends"), "No Mutual Friends");
	assert_eq!(app.text("full-profile-tab-servers"), "No Mutual Servers");
	assert!(app.find("full-profile-no-activity").is_some());
	assert!(
		app.texts()
			.iter()
			.any(|text| text == "wing doesn't have any activity to share here")
	);
	app.click("full-profile-tab-servers");
	app.frames(5);
	assert_eq!(app.text("full-profile-empty"), "No mutual servers");
	app.click("full-profile-more");
	app.frames(10);
	assert!(app.find("profile-menu-share").is_some());
	assert!(
		app.find("profile-menu-full-profile").is_none(),
		"the full profile is already open"
	);
	app.key(zgx::input::Key::Escape);
	app.frames(5);
	app.click("full-profile-close");
	app.frames(10);
	assert!(app.find("full-profile").is_none(), "the close button closes it");
}

#[test]
fn the_dm_columns_more_opens_the_same_menu_at_the_pointer() {
	let tokio = tokio();
	let (mut app, (layer, requests)) = profile_app(&tokio);
	layer.open_menu(WING, (900., 120.));
	layer.show_card(card_of(&wing_state(), false));
	app.frames(10);
	assert!(app.find("profile-popout").is_none(), "no popout, only the menu");
	for item in ["full-profile", "share", "ignore", "block", "report", "copy-id"] {
		assert!(
			app.find(&format!("profile-menu-{item}")).is_some(),
			"the menu shows {item}"
		);
	}
	let (mut card, menu) = card_of(&wing_state(), false).expect("a card");
	card.bio = Some("the profile arrived".to_owned());
	layer.show_card(Some((card, menu)));
	app.frames(10);
	assert!(layer.target().is_some(), "a changed card keeps the menu open");
	assert!(app.find("profile-menu-share").is_some());
	app.click("profile-menu-full-profile");
	app.frames(5);
	assert!(
		requests
			.borrow()
			.iter()
			.any(|request| request.contains("FullProfile")),
		"View Full Profile asks for it: {:?}",
		requests.borrow()
	);
	assert!(layer.target().is_none(), "choosing closes the menu");
}
