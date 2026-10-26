use std::rc::Rc;

use bridge::StatusChoice;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};

use super::parts::{self, label};
use super::{AccountRow, AccountsShown, MenuShown, ProfileLayer, Request};
use crate::kit::separator;
use crate::timeline::icons::icon;
use crate::timeline::tok;

const LEADING: f32 = 20.;
const AVATAR: f32 = 40.;
const DOT: f32 = 12.;
const REMOVE: f32 = 24.;
const CHOSEN: f32 = 16.;
const CHOSEN_ICON: f32 = 12.;

pub fn menu(layer: &Rc<ProfileLayer>, shown: &MenuShown) -> View {
	let frame = column()
		.w(tok("status_menu_width"))
		.px(tok("space_8"))
		.py(tok("space_6"))
		.gap(tok("space_2"))
		.rounded(tok("radius_sm"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_3"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		});
	match &shown.accounts {
		Some(accounts) => frame.children(accounts_view(layer, accounts)),
		None => frame.children(main_view(layer, shown)),
	}
}

fn main_view(layer: &Rc<ProfileLayer>, shown: &MenuShown) -> Vec<View> {
	let panel = shown.panel.clone();
	let pictures = layer.pictures.clone();
	let choices: Vec<View> = StatusChoice::ALL
		.into_iter()
		.map(|choice| {
			let layer = layer.clone();
			entry(
				parts::status_dot(choice.dot(), DOT, "bg_3"),
				choice.label(),
				false,
			)
			.when(choice == panel.status, |entry| entry.child(chosen()))
			.on_click(move || layer.choose_status(choice))
		})
		.collect();
	let custom_label = panel
		.custom_status
		.text
		.clone()
		.unwrap_or_else(|| "Set Custom Status".to_owned());
	let (edit, custom, switch, sign_out) = (layer.clone(), layer.clone(), layer.clone(), layer.clone());
	let mut view = vec![
		row()
			.items_center()
			.gap(8.)
			.p(8.)
			.child(parts::avatar(
				&pictures,
				shown.avatar.as_ref(),
				AVATAR,
				Some(panel.status.dot()),
				"bg_3",
			))
			.child(
				column()
					.min_w(0.)
					.child(
						label(panel.name.clone(), "font_md")
							.font_weight(700)
							.truncate()
							.text_color(tok("text_2")),
					)
					.child(
						label(panel.username.clone(), "font_xs")
							.truncate()
							.text_color(tok("text_4")),
					),
			),
		separator(),
		entry(icon("profile", LEADING, "text_4"), "Edit Profile", false).on_click(move || {
			edit.menu.set(None);
			edit.request(Request::EditProfile);
		}),
		separator(),
	];
	view.extend(choices);
	view.extend([
		separator(),
		entry(icon("emoji", LEADING, "text_4"), &custom_label, false)
			.on_click(move || custom.open_custom_status()),
		separator(),
		entry(icon("account", LEADING, "text_4"), "Switch Accounts", false)
			.on_click(move || switch.request(Request::SwitchAccounts)),
		entry(icon("log_out", LEADING, "red_2"), "Log Out", true).on_click(move || {
			sign_out.menu.set(None);
			sign_out.request(Request::SignOut);
		}),
	]);
	view
}

fn accounts_view(layer: &Rc<ProfileLayer>, accounts: &AccountsShown) -> Vec<View> {
	let (back, add, manage) = (layer.clone(), layer.clone(), layer.clone());
	let mut view = vec![
		entry(icon("chevron_left", LEADING, "text_4"), "Switch Accounts", false)
			.on_click(move || back.close_accounts()),
		separator(),
	];
	view.extend(
		accounts
			.rows
			.iter()
			.map(|account| account_row(layer, account, accounts.managing)),
	);
	let (manage_label, manage_icon) = if accounts.managing {
		("Done", "check")
	} else {
		("Manage Accounts", "user_cog")
	};
	view.extend([
		separator(),
		entry(icon("plus", LEADING, "text_4"), "Add an Account", false).on_click(move || {
			add.menu.set(None);
			add.request(Request::AddAccount);
		}),
		entry(icon(manage_icon, LEADING, "text_4"), manage_label, false)
			.on_click(move || manage.toggle_managing()),
	]);
	view
}

fn account_row(layer: &Rc<ProfileLayer>, account: &AccountRow, managing: bool) -> View {
	let leading = parts::avatar(&layer.pictures, account.avatar.as_ref(), LEADING, None, "bg_3");
	let entry = entry(leading, &account.name, false);
	let user_id = account.user_id;
	if managing {
		if account.current {
			return entry;
		}
		let remove = layer.clone();
		return entry.child(
			row()
				.size(REMOVE, REMOVE)
				.shrink_0()
				.items_center()
				.justify_center()
				.rounded(tok("radius_sm"))
				.cursor(Cursor::Pointer)
				.hover(|style| style.bg(tok("background_feedback_critical")))
				.child(icon("trash", LEADING - 4., "red_2"))
				.on_click(move || remove.request(Request::RemoveAccount(user_id))),
		);
	}
	let switch = layer.clone();
	entry
		.when(account.current, |entry| {
			entry.child(icon("check", LEADING, "text_4"))
		})
		.on_click(move || {
			switch.menu.set(None);
			switch.request(Request::SwitchTo(user_id));
		})
}

fn entry(leading: View, caption: &str, danger: bool) -> View {
	let (color, wash) = if danger {
		("red_2", "background_feedback_critical")
	} else {
		("text_3", "hover")
	};
	row()
		.items_center()
		.gap(tok("space_8"))
		.h(tok("menu_item_height"))
		.px(tok("space_8"))
		.rounded(tok("radius_xs"))
		.cursor(Cursor::Pointer)
		.text_color(tok(color))
		.hover(move |style| style.bg(tok(wash)))
		.child(row().w(LEADING).shrink_0().justify_center().child(leading))
		.child(
			label(caption.to_owned(), "font_sm")
				.font_weight(500)
				.grow()
				.min_w(0.)
				.truncate(),
		)
}

fn chosen() -> View {
	row()
		.size(CHOSEN, CHOSEN)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(CHOSEN)
		.bg(tok("accent_2"))
		.child(icon("check", CHOSEN_ICON, "text_0"))
}
