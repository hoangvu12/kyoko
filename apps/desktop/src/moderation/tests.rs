use std::cell::RefCell;
use std::rc::Rc;

use bridge::{ModerationItem, RoleChoice};
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::scene::Rect;
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{MemberMenu, ModerationLayer, Request};

const WINDOW: (f32, f32) = (1100., 720.);
const SCREEN: Rect = Rect {
	x: -400.,
	y: -200.,
	width: 2560.,
	height: 1440.,
};
const GUILD: Snowflake = Snowflake(10);
const MEMBER: Snowflake = Snowflake(30);
const HELD: Snowflake = Snowflake(41);

type Heard = Rc<RefCell<Vec<Request>>>;

fn roles(count: u64) -> Vec<RoleChoice> {
	(0..count)
		.map(|index| RoleChoice {
			id: Snowflake(HELD.0 + index),
			name: format!("Role {index}"),
			held: index == 0,
		})
		.collect()
}

fn member_menu(roles: Vec<RoleChoice>) -> MemberMenu {
	MemberMenu {
		guild_id: GUILD,
		user_id: MEMBER,
		name: "Brave Ember".to_owned(),
		nick: String::new(),
		head: true,
		developer: false,
		items: vec![
			ModerationItem::Nickname,
			ModerationItem::Roles(roles),
			ModerationItem::Separator,
			ModerationItem::Timeout,
			ModerationItem::Kick,
			ModerationItem::Ban,
		],
	}
}

fn app() -> (TestApp, Rc<ModerationLayer>, Heard) {
	let made: Rc<RefCell<Option<Rc<ModerationLayer>>>> = Rc::default();
	let heard: Heard = Rc::default();
	let (into, used, hook) = (made.clone(), made.clone(), heard.clone());
	let app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let layer = ModerationLayer::new(runtime);
			layer.set_window(WINDOW.0, WINDOW.1);
			layer.on_request(move |request| hook.borrow_mut().push(request));
			*into.borrow_mut() = Some(layer);
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |luau| {
			let layer = used.borrow().clone().expect("the moderation layer");
			overlay()
				.w_full()
				.h_full()
				.child(luau)
				.child(div().absolute().left(0.).top(0.).w(0.).h(0.).child(layer.view()))
		},
	);
	app.ui.scene.borrow_mut().resize(WINDOW.0, WINDOW.1);
	app.ui.set_popup_screens(vec![SCREEN]);
	let layer = made.borrow().clone().expect("the moderation layer");
	(app, layer, heard)
}

fn hover(app: &mut TestApp, id: &str) {
	let bounds = app.bounds(id);
	app.ui.dispatch(InputEvent::PointerMove {
		x: bounds.x + bounds.width / 2.,
		y: bounds.y + bounds.height / 2.,
	});
	app.frames(10);
}

fn inside_window(bounds: Rect) -> bool {
	bounds.x >= 0.
		&& bounds.y >= 0.
		&& bounds.x + bounds.width <= WINDOW.0
		&& bounds.y + bounds.height <= WINDOW.1
}

fn roles_submenu_at(at: (f32, f32), count: u64) -> (Rect, Rect) {
	let (mut app, layer, _) = app();
	assert!(layer.open_menu(member_menu(roles(count)), at));
	app.frames(10);
	hover(&mut app, "moderation-roles");
	assert!(
		app.find("moderation-roles-menu").is_some(),
		"hovering Roles opens its submenu"
	);
	(
		app.bounds("moderation-member-menu"),
		app.bounds("moderation-roles-menu"),
	)
}

#[test]
fn the_roles_submenu_from_a_member_row_at_the_right_edge_stays_in_the_window() {
	let (menu, submenu) = roles_submenu_at((WINDOW.0 - 40., 180.), 4);
	assert!(inside_window(menu), "the member menu is in the window: {menu:?}");
	assert!(
		inside_window(submenu),
		"the Roles submenu is in the window: {submenu:?}"
	);
	assert!(
		submenu.x < menu.x && submenu.x + submenu.width < menu.x + menu.width / 2.,
		"the Roles submenu flips to the left of the member menu: {menu:?} {submenu:?}"
	);
	let (menu, submenu) = roles_submenu_at((400., 180.), 4);
	assert!(
		submenu.x > menu.x + menu.width / 2.,
		"with room on the right, the Roles submenu opens to the right: {menu:?} {submenu:?}"
	);
	let (_, submenu) = roles_submenu_at((WINDOW.0 - 40., WINDOW.1 - 40.), 30);
	assert!(
		inside_window(submenu),
		"a long Roles submenu from the bottom right stays in the window: {submenu:?}"
	);
}

#[test]
fn a_held_role_is_checked_and_choosing_it_removes_it() {
	let (mut app, layer, heard) = app();
	assert!(layer.open_menu(member_menu(roles(3)), (WINDOW.0 - 40., 180.)));
	app.frames(10);
	hover(&mut app, "moderation-roles");
	let checked = |app: &mut TestApp, role: Snowflake| {
		let node = app
			.find(&format!("moderation-role-{role}"))
			.expect("the role row");
		app.ui
			.semantics
			.borrow()
			.get(node)
			.and_then(|semantic| semantic.checked)
	};
	assert_eq!(checked(&mut app, HELD), Some(true));
	assert_eq!(checked(&mut app, Snowflake(HELD.0 + 1)), Some(false));
	app.click(&format!("moderation-role-{HELD}"));
	app.wait_until(|_| !heard.borrow().is_empty());
	let heard = heard.borrow();
	assert!(
		matches!(
			heard.as_slice(),
			[Request::Command(Command::RemoveMemberRole {
				guild_id: GUILD,
				user_id: MEMBER,
				role_id: HELD,
			})]
		),
		"choosing a held role removes it: {heard:?}"
	);
}
