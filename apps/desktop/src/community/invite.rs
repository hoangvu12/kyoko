use std::rc::Rc;

use bridge::{DEFAULT_INVITE_AGE, INVITE_AGES, INVITE_USES};
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};

use super::parts::{self, Tone, action_button, body, field, footer, labelled_button, panel, switch_row};
use super::{CommunityLayer, Request};
use crate::timeline::{TypeSize, icon, tok};

const WIDTH: f32 = 440.;
const MENU_ITEM: f32 = 28.;
const LINK_HEIGHT: f32 = 36.;

type Options = (usize, usize, bool);

#[derive(Debug, Clone, PartialEq)]
enum Wait {
	Idle,
	For(Option<String>, Options),
	Shown(String, Options),
}

pub struct Waiting {
	wait: Signal<Wait>,
}

impl Waiting {
	pub fn new(runtime: &Runtime) -> Self {
		Self {
			wait: runtime.signal(Wait::Idle),
		}
	}

	pub fn reset(&self) {
		self.wait.set(Wait::Idle);
	}

	fn generate(&self, current: Option<String>, options: Options) {
		self.wait.set(Wait::For(current, options));
	}

	pub fn arrived(&self, code: Option<String>) {
		let fresh = self.wait.with_untracked(|wait| match (wait, &code) {
			(Wait::For(before, options), Some(code)) if before.as_ref() != Some(code) => {
				Some(Wait::Shown(code.clone(), *options))
			}
			_ => None,
		});
		if let Some(fresh) = fresh {
			self.wait.set(fresh);
		}
	}
}

pub fn view(layer: &Rc<CommunityLayer>, channel_id: Snowflake) -> View {
	let runtime = &layer.runtime;
	let age = runtime.signal(DEFAULT_INVITE_AGE);
	let uses = runtime.signal(0usize);
	let temporary = runtime.signal(false);
	let wait = layer.invite.wait.clone();
	let options = {
		let (age, uses, temporary) = (age.clone(), uses.clone(), temporary.clone());
		move || (age.get(), uses.get(), temporary.get())
	};
	let shown = {
		let (wait, options) = (wait.clone(), options.clone());
		move || match wait.get() {
			Wait::Shown(code, chosen) if chosen == options() => Some(code),
			_ => None,
		}
	};
	let generate = {
		let (layer, age, uses, temporary) = (layer.clone(), age.clone(), uses.clone(), temporary.clone());
		move || {
			let current = layer
				.read(|state| state.invite_created(channel_id).map(|invite| invite.code.clone()))
				.flatten();
			let options = (
				age.with_untracked(|age| *age),
				uses.with_untracked(|uses| *uses),
				temporary.with_untracked(|temporary| *temporary),
			);
			layer.invite.generate(current, options);
			layer.send(Command::CreateInvite {
				channel_id,
				max_age: INVITE_AGES[age.with_untracked(|age| *age)].1,
				max_uses: INVITE_USES[uses.with_untracked(|uses| *uses)].1,
				temporary: temporary.with_untracked(|temporary| *temporary),
			});
		}
	};
	let label = {
		let shown = shown.clone();
		text_signal(move || {
			if shown().is_some() {
				"Generate New Link".to_owned()
			} else {
				"Generate Link".to_owned()
			}
		})
	};
	let (close, cancel, copier) = (layer.clone(), layer.clone(), layer.clone());
	panel(
		WIDTH,
		"Invite People",
		move || close.close(),
		body()
			.child(field(
				"Expire after",
				select(layer, "Expire after", &INVITE_AGES.map(|(label, _)| label), age),
			))
			.child(field(
				"Max number of uses",
				select(
					layer,
					"Max number of uses",
					&INVITE_USES.map(|(label, _)| label),
					uses,
				),
			))
			.child(switch_row(
				"Grant temporary membership",
				Some("Members who have not been given a role are removed when they disconnect."),
				temporary,
			))
			.child_when(shown, move |code: String, _| link_row(&copier, &code))
			.child(footer(
				"Close",
				labelled_button(label, Tone::Primary, generate),
				move || cancel.close(),
			)),
	)
}

fn link_row(layer: &Rc<CommunityLayer>, code: &str) -> View {
	let link = bridge::invite_link(code);
	let copy = link.clone();
	let layer = layer.clone();
	field(
		"Invite link",
		row()
			.w_full()
			.items_center()
			.gap(8.)
			.child(
				text(link)
					.grow()
					.min_w(0.)
					.truncate()
					.h(LINK_HEIGHT)
					.px(12.)
					.items_center()
					.rounded(tok("radius_sm"))
					.bg(tok("input_background"))
					.type_size("font_md")
					.line_height(LINK_HEIGHT)
					.text_color(tok("text_normal")),
			)
			.child(action_button("Copy", Tone::Primary, move || {
				layer.request(Request::Copy(copy.clone()));
			})),
	)
}

fn select(layer: &Rc<CommunityLayer>, label: &str, options: &[&str], chosen: Signal<usize>) -> View {
	let open = layer.runtime.signal(false);
	let toggle = open.clone();
	let options: Vec<String> = options.iter().map(|option| (*option).to_owned()).collect();
	let current = {
		let (chosen, options) = (chosen.clone(), options.clone());
		text_signal(move || options[chosen.get()].clone())
	};
	let anchor = button()
		.h(parts::BUTTON_HEIGHT)
		.px(12.)
		.gap(8.)
		.items_center()
		.border(1.)
		.border_color(tok("divider"))
		.rounded(tok("radius_sm"))
		.bg(parts::clear())
		.hover(|style| style.bg(tok("modifier_hover")))
		.cursor(Cursor::Pointer)
		.text_size(tok("font_sm"))
		.font_weight(500)
		.text_color(tok("text_normal"))
		.child(current)
		.child(icon("chevron_down", 12., "interactive_normal"))
		.on_click(move || {
			toggle.set(!toggle.with_untracked(|open| *open));
		});
	let items = options.into_iter().enumerate().map(|(index, option)| {
		let (pick, checked) = (chosen.clone(), chosen.clone());
		menu_item(option.clone())
			.h(MENU_ITEM)
			.px(12.)
			.py(0.)
			.rounded(tok("radius_sm"))
			.child(
				row()
					.w_full()
					.items_center()
					.justify_between()
					.gap(8.)
					.child(
						text(option)
							.text_size(tok("font_sm"))
							.text_color(tok("text_normal")),
					)
					.child(
						div()
							.child(icon("check", 12., "text_normal"))
							.reactive_style(move || {
								if checked.get() == index {
									Styles::new().visible()
								} else {
									Styles::new().invisible()
								}
							}),
					),
			)
			.on_click(move || {
				pick.set(index);
			})
	});
	row().child(
		menu(label.to_owned(), open, anchor)
			.px(0.)
			.py(4.)
			.rounded(tok("radius_sm"))
			.bg(tok("background_floating"))
			.border(1.)
			.border_color(tok("divider"))
			.children(items.collect::<Vec<_>>()),
	)
}
