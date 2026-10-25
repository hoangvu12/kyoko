use std::rc::Rc;

use bridge::StatusChoice;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};

use super::parts::{self, label};
use super::{AccountRow, AccountsShown, MenuShown, ProfileLayer, Request};
use crate::timeline::icons::icon;
use crate::timeline::tok;

const WIDTH: f32 = 260.;
const ENTRY: f32 = 32.;
const LEADING: f32 = 20.;
const AVATAR: f32 = 40.;
const DOT: f32 = 12.;
const REMOVE: f32 = 24.;

pub fn menu(layer: &Rc<ProfileLayer>, shown: &MenuShown) -> View {
	let frame = column()
		.w(WIDTH)
		.p(8.)
		.gap(2.)
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok("background_floating"))
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
				parts::status_dot(choice.dot(), DOT, "background_floating"),
				choice.label(),
				false,
			)
			.when(choice == panel.status, |entry| entry.bg(tok("modifier_hover")))
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
				"background_floating",
			))
			.child(
				column()
					.min_w(0.)
					.child(
						label(panel.name.clone(), "font_md")
							.font_weight(700)
							.truncate()
							.text_color(tok("header_primary")),
					)
					.child(
						label(panel.username.clone(), "font_xs")
							.truncate()
							.text_color(tok("text_muted")),
					),
			),
		parts::spaced_divider(),
		entry(
			icon("profile", LEADING, "interactive_normal"),
			"Edit Profile",
			false,
		)
		.on_click(move || {
			edit.menu.set(None);
			edit.request(Request::EditProfile);
		}),
		parts::spaced_divider(),
	];
	view.extend(choices);
	view.extend([
		parts::spaced_divider(),
		entry(icon("emoji", LEADING, "interactive_normal"), &custom_label, false)
			.on_click(move || custom.open_custom_status()),
		parts::spaced_divider(),
		entry(
			icon("account", LEADING, "interactive_normal"),
			"Switch Accounts",
			false,
		)
		.on_click(move || switch.request(Request::SwitchAccounts)),
		entry(icon("log_out", LEADING, "danger"), "Log Out", true).on_click(move || {
			sign_out.menu.set(None);
			sign_out.request(Request::SignOut);
		}),
	]);
	view
}

fn accounts_view(layer: &Rc<ProfileLayer>, accounts: &AccountsShown) -> Vec<View> {
	let (back, add, manage) = (layer.clone(), layer.clone(), layer.clone());
	let mut view = vec![
		entry(
			icon("chevron_left", LEADING, "interactive_normal"),
			"Switch Accounts",
			false,
		)
		.on_click(move || back.close_accounts()),
		parts::spaced_divider(),
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
		parts::spaced_divider(),
		entry(
			icon("plus", LEADING, "interactive_normal"),
			"Add an Account",
			false,
		)
		.on_click(move || {
			add.menu.set(None);
			add.request(Request::AddAccount);
		}),
		entry(
			icon(manage_icon, LEADING, "interactive_normal"),
			manage_label,
			false,
		)
		.on_click(move || manage.toggle_managing()),
	]);
	view
}

fn account_row(layer: &Rc<ProfileLayer>, account: &AccountRow, managing: bool) -> View {
	let leading = parts::avatar(
		&layer.pictures,
		account.avatar.as_ref(),
		LEADING,
		None,
		"background_floating",
	);
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
				.hover(|style| style.bg(tok("modifier_hover")))
				.child(icon("trash", LEADING - 4., "danger"))
				.on_click(move || remove.request(Request::RemoveAccount(user_id))),
		);
	}
	let switch = layer.clone();
	entry
		.when(account.current, |entry| {
			entry.child(icon("check", LEADING, "interactive_normal"))
		})
		.on_click(move || {
			switch.menu.set(None);
			switch.request(Request::SwitchTo(user_id));
		})
}

fn entry(leading: View, caption: &str, danger: bool) -> View {
	let (rest, hover) = if danger {
		("danger", "danger")
	} else {
		("interactive_normal", "interactive_active")
	};
	row()
		.items_center()
		.gap(8.)
		.h(ENTRY)
		.px(8.)
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.text_color(tok(rest))
		.hover(move |style| style.bg(tok("modifier_selected")).text_color(tok(hover)))
		.child(row().w(LEADING).shrink_0().justify_center().child(leading))
		.child(
			label(caption.to_owned(), "font_sm")
				.font_weight(500)
				.grow()
				.min_w(0.)
				.truncate(),
		)
}
