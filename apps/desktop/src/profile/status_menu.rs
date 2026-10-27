use std::rc::Rc;

use bridge::StatusChoice;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};

use super::parts::{self, Badge, words};
use super::{AccountRow, AccountsShown, MenuShown, ProfileLayer, Request, Submenu};
use crate::kit::{MENU_ITEM, id_badge, separator};
use crate::timeline::icons::icon;
use crate::timeline::tok;

const LEADING: f32 = 16.;
const ACCOUNT_AVATAR: f32 = 20.;
const CHOSEN_ICON: f32 = 12.;

fn stop_presses(view: View) -> View {
	view.on_event(|event| {
		if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
			event.stop_propagation();
		}
	})
}

pub fn menu(layer: &Rc<ProfileLayer>, shown: &MenuShown) -> View {
	let submenu = layer.submenu.clone();
	let this = layer.clone();
	let accounts = shown.accounts.clone();
	row()
		.items_end()
		.gap(tok("space_8"))
		.child(account_popout(layer, shown))
		.child(div().child_when(
			move || Some(submenu.get()),
			move |open: Submenu, _| match (open, accounts.as_ref()) {
				(Submenu::Statuses, _) => statuses(&this),
				(Submenu::Accounts, Some(accounts)) => accounts_menu(&this, accounts),
				_ => div(),
			},
		))
}

fn menu_card(view: View) -> View {
	stop_presses(view)
		.px(tok("space_8"))
		.py(tok("space_6"))
		.gap(tok("space_2"))
		.rounded(tok("radius_sm"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
}

fn account_popout(layer: &Rc<ProfileLayer>, shown: &MenuShown) -> View {
	let panel = shown.panel.clone();
	let pictures = layer.pictures.clone();
	let badge = Badge::of(&layer.runtime, panel.status.dot(), "_lg", 0.);
	let current = panel.status;
	let custom_label = panel
		.custom_status
		.text
		.clone()
		.unwrap_or_else(|| "Set Custom Status".to_owned());
	let (edit, custom, copy) = (layer.clone(), layer.clone(), layer.clone());
	let (status_hover, status_click) = (layer.clone(), layer.clone());
	let (switch_hover, switch_click) = (layer.clone(), layer.clone());
	let user_id = panel.user.id;
	let leaving = |layer: &Rc<ProfileLayer>| {
		let layer = layer.clone();
		move || layer.show_submenu(Submenu::None)
	};
	let items = column()
		.id("account-popout-menu")
		.w_full()
		.bg(tok("bg_4"))
		.child(
			entry(icon("edit", LEADING, "text_4"), "Edit Profile", None)
				.id("status-menu-edit-profile")
				.on_hover_enter(leaving(layer))
				.on_click(move || {
					edit.menu.set(None);
					edit.request(Request::EditProfile);
				}),
		)
		.child(separator())
		.child(
			entry(
				dot(layer, current),
				current.label(),
				Some(icon("chevron_right", LEADING, "text_4")),
			)
			.id("status-menu-status")
			.on_hover_enter(move || status_hover.show_submenu(Submenu::Statuses))
			.on_click(move || status_click.show_submenu(Submenu::Statuses)),
		)
		.child(
			entry(icon("emoji", LEADING, "text_4"), &custom_label, None)
				.id("status-menu-custom-status")
				.on_hover_enter(leaving(layer))
				.on_click(move || custom.open_custom_status()),
		)
		.child(separator())
		.child(
			entry(
				icon("refresh", LEADING, "text_4"),
				"Switch Accounts",
				Some(icon("chevron_right", LEADING, "text_4")),
			)
			.id("status-menu-switch-accounts")
			.on_hover_enter(move || switch_hover.open_accounts())
			.on_click(move || switch_click.open_accounts()),
		)
		.child(separator())
		.child(
			entry(div(), "Copy User ID", Some(id_badge()))
				.id("status-menu-copy-id")
				.on_hover_enter(leaving(layer))
				.on_click(move || {
					copy.menu.set(None);
					crate::toasts::copy(&user_id.to_string(), "User ID copied");
				}),
		);
	column()
		.id("account-popout")
		.relative()
		.w(tok("popout_width"))
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_3"))
		.overflow_hidden()
		.text_color(tok("text_3"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.child(parts::banner(
			&pictures,
			panel.accent(),
			shown.banner.as_ref(),
			"account_banner_height",
		))
		.child(
			column()
				.w_full()
				.pt(tok("account_body_top"))
				.px(tok("space_16"))
				.pb(tok("space_16"))
				.gap(tok("space_12"))
				.child(
					column()
						.w_full()
						.child(
							words(panel.name.clone(), "lg")
								.font_weight(700)
								.truncate()
								.text_color(tok("text_2")),
						)
						.child(words(panel.username.clone(), "sm").truncate()),
				)
				.child(menu_card(items)),
		)
		.child(
			div()
				.absolute()
				.left(parts::size(&layer.runtime, "space_16"))
				.top(parts::size(&layer.runtime, "account_avatar_top"))
				.child(parts::avatar_ring(
					"profile_avatar_ring",
					"space_6",
					zgx::theme::Themed::Token(tok("bg_3")),
					parts::avatar(
						&pictures,
						shown.avatar.as_ref(),
						parts::size(&layer.runtime, "avatar_size_lg"),
						Some(badge),
						"bg_3",
					),
				)),
		)
}

fn dot(layer: &Rc<ProfileLayer>, choice: StatusChoice) -> View {
	parts::status_dot(choice.dot(), parts::size(&layer.runtime, "status_size"), "bg_3")
}

fn statuses(layer: &Rc<ProfileLayer>) -> View {
	let current = layer.panel.borrow().as_ref().map(|panel| panel.status);
	menu_card(column())
		.id("status-submenu")
		.w(tok("status_menu_width"))
		.bg(tok("bg_3"))
		.children(
			StatusChoice::ALL
				.into_iter()
				.map(|choice| {
					let pick = layer.clone();
					noted_entry(
						dot(layer, choice),
						choice.label(),
						choice.description(),
						(current == Some(choice)).then(chosen),
					)
					.id(format!("status-choice-{}", choice.key()))
					.on_click(move || pick.choose_status(choice))
				})
				.collect::<Vec<_>>(),
		)
}

fn accounts_menu(layer: &Rc<ProfileLayer>, accounts: &AccountsShown) -> View {
	let (add, manage) = (layer.clone(), layer.clone());
	let (manage_label, manage_icon) = if accounts.managing {
		("Done", "check")
	} else {
		("Manage Accounts", "user_cog")
	};
	menu_card(column())
		.id("accounts-submenu")
		.w(tok("status_menu_width"))
		.bg(tok("bg_3"))
		.children(
			accounts
				.rows
				.iter()
				.map(|account| account_row(layer, account, accounts.managing))
				.collect::<Vec<_>>(),
		)
		.child(separator())
		.child(
			entry(icon("plus", LEADING, "text_4"), "Add an Account", None).on_click(move || {
				add.menu.set(None);
				add.request(Request::AddAccount);
			}),
		)
		.child(
			entry(icon(manage_icon, LEADING, "text_4"), manage_label, None)
				.on_click(move || manage.toggle_managing()),
		)
}

fn account_row(layer: &Rc<ProfileLayer>, account: &AccountRow, managing: bool) -> View {
	let leading = parts::avatar(
		&layer.pictures,
		account.avatar.as_ref(),
		ACCOUNT_AVATAR,
		None,
		"bg_3",
	);
	let user_id = account.user_id;
	if managing {
		if account.current {
			return entry(leading, &account.name, None);
		}
		let remove = layer.clone();
		let trash = row()
			.w(tok("icon_size_lg"))
			.h(tok("icon_size_lg"))
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_xs"))
			.cursor(Cursor::Pointer)
			.hover(|style| style.bg(tok("background_feedback_critical")))
			.child(icon("trash", LEADING, "red_2"))
			.on_click(move || remove.request(Request::RemoveAccount(user_id)));
		return entry(leading, &account.name, Some(trash));
	}
	let switch = layer.clone();
	entry(
		leading,
		&account.name,
		account.current.then(|| icon("check", LEADING, "text_4")),
	)
	.on_click(move || {
		switch.menu.set(None);
		switch.request(Request::SwitchTo(user_id));
	})
}

trait Hovered {
	fn on_hover_enter(self, action: impl Fn() + 'static) -> Self;
}

impl Hovered for View {
	fn on_hover_enter(self, action: impl Fn() + 'static) -> Self {
		self.on_event(move |event| {
			if matches!(event.event, InputEvent::PointerEnter) {
				action();
			}
		})
	}
}

fn entry(leading: View, caption: &str, trailing: Option<View>) -> View {
	noted_entry(leading, caption, None, trailing)
}

fn noted_entry(leading: View, caption: &str, note: Option<&str>, trailing: Option<View>) -> View {
	row()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.min_h(MENU_ITEM)
		.px(tok("space_8"))
		.py(tok("space_6"))
		.rounded(tok("radius_xs"))
		.cursor(Cursor::Pointer)
		.text_color(tok("text_3"))
		.hover(|style| style.bg(tok("hover")))
		.child(row().w(LEADING).shrink_0().justify_center().child(leading))
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					words(caption.to_owned(), "sm")
						.font_weight(500)
						.min_w(0.)
						.truncate(),
				)
				.when_some(note.map(str::to_owned), |label, note| {
					label.child(words(note, "xs").text_wrap(true).text_color(tok("text_4")))
				}),
		)
		.when_some(trailing, |item, trailing| item.child(trailing))
}

fn chosen() -> View {
	row()
		.w(tok("icon_size_sm"))
		.h(tok("icon_size_sm"))
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("icon_size_sm"))
		.bg(tok("accent_2"))
		.child(icon("check", CHOSEN_ICON, "text_0"))
}
