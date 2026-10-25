use std::rc::Rc;

use bridge::StatusChoice;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};

use super::parts::{self, label};
use super::{MenuShown, ProfileLayer, Request};
use crate::timeline::icons::icon;
use crate::timeline::tok;

const WIDTH: f32 = 260.;
const ENTRY: f32 = 32.;
const LEADING: f32 = 20.;
const AVATAR: f32 = 40.;
const DOT: f32 = 12.;

pub fn menu(layer: &Rc<ProfileLayer>, shown: &MenuShown) -> View {
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
	column()
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
		})
		.child(
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
		)
		.child(parts::spaced_divider())
		.child(
			entry(
				icon("profile", LEADING, "interactive_normal"),
				"Edit Profile",
				false,
			)
			.on_click(move || {
				edit.menu.set(None);
				edit.request(Request::EditProfile);
			}),
		)
		.child(parts::spaced_divider())
		.children(choices)
		.child(parts::spaced_divider())
		.child(
			entry(icon("emoji", LEADING, "interactive_normal"), &custom_label, false)
				.on_click(move || custom.open_custom_status()),
		)
		.child(parts::spaced_divider())
		.child(
			entry(
				icon("account", LEADING, "interactive_normal"),
				"Switch Accounts",
				false,
			)
			.on_click(move || {
				switch.menu.set(None);
				switch.request(Request::SwitchAccounts);
			}),
		)
		.child(
			entry(icon("log_out", LEADING, "danger"), "Log Out", true).on_click(move || {
				sign_out.menu.set(None);
				sign_out.request(Request::SignOut);
			}),
		)
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
