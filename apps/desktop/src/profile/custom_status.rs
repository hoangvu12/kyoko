use std::cell::Cell;
use std::rc::Rc;

use bridge::StatusLine;
use chrono::{DateTime, Duration, Local, TimeDelta, Utc};
use session::Command;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::{Runtime, Signal};
use zgx::scene::Color;
use zgx::theme::Themed;

use super::parts::{self, label};
use super::{ProfileLayer, Request};
use crate::emoji;
use crate::timeline::icons::icon;
use crate::timeline::tok;

const WIDTH: f32 = 440.;
const FIELD: f32 = 36.;
const INPUT: f32 = 32.;
const CLOSE: f32 = 16.;
const CHIP: f32 = 32.;
const BUTTON: f32 = 32.;
const MAX_LENGTH: usize = 128;
const TOP_SHARE: f32 = 0.1;
const STATUS_GLYPH: f32 = 24.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearAfter {
	Today,
	FourHours,
	OneHour,
	ThirtyMinutes,
	Never,
}

impl ClearAfter {
	pub const ALL: [ClearAfter; 5] = [
		Self::Today,
		Self::FourHours,
		Self::OneHour,
		Self::ThirtyMinutes,
		Self::Never,
	];

	pub fn label(self) -> &'static str {
		match self {
			Self::Today => "Today",
			Self::FourHours => "4 hours",
			Self::OneHour => "1 hour",
			Self::ThirtyMinutes => "30 minutes",
			Self::Never => "Don't clear",
		}
	}

	pub fn expires(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
		match self {
			Self::Never => None,
			Self::ThirtyMinutes => Some(now + Duration::minutes(30)),
			Self::OneHour => Some(now + Duration::hours(1)),
			Self::FourHours => Some(now + Duration::hours(4)),
			Self::Today => {
				let tomorrow = now.with_timezone(&Local).date_naive().succ_opt()?;
				let midnight = tomorrow.and_hms_opt(0, 0, 0)?;
				let local = midnight.and_local_timezone(Local).earliest()?;
				Some(local.with_timezone(&Utc)).filter(|end| *end - now > TimeDelta::zero())
			}
		}
	}
}

pub struct CustomStatusForm {
	open: Signal<bool>,
	text: Signal<String>,
	emoji: Signal<Option<String>>,
	clear_after: Signal<ClearAfter>,
	spot: Cell<(f32, f32)>,
}

impl CustomStatusForm {
	pub fn new(runtime: &Runtime) -> Self {
		Self {
			open: runtime.signal(false),
			text: runtime.signal(String::new()),
			emoji: runtime.signal(None),
			clear_after: runtime.signal(ClearAfter::Today),
			spot: Cell::new((0., 0.)),
		}
	}

	pub fn open(&self, current: StatusLine) {
		self.text.set(current.text.unwrap_or_default());
		self.emoji
			.set(current.emoji_id.is_none().then_some(current.emoji_name).flatten());
		self.clear_after.set(ClearAfter::Today);
		self.open.set(true);
	}

	pub fn close(&self) {
		self.open.set(false);
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn open_signal(&self) -> Signal<bool> {
		self.open.clone()
	}

	pub fn set_emoji(&self, glyph: Option<String>) {
		self.emoji.set(glyph);
	}

	pub fn emoji_spot(&self) -> (f32, f32) {
		self.spot.get()
	}

	pub fn command(&self, now: DateTime<Utc>) -> Command {
		let text: String = self
			.text
			.with_untracked(|text| text.trim().chars().take(MAX_LENGTH).collect());
		let text = (!text.is_empty()).then_some(text);
		let emoji = self.emoji.with_untracked(Clone::clone);
		let content = text.is_some() || emoji.is_some();
		let clear_after = self.clear_after.with_untracked(|choice| *choice);
		Command::SetCustomStatus {
			text,
			emoji,
			expires: content.then(|| clear_after.expires(now)).flatten(),
		}
	}
}

pub fn dialog(layer: &Rc<ProfileLayer>) -> View {
	let form = layer.custom.clone();
	let open = form.open.clone();
	let this = layer.clone();
	div().child_when(move || open.get().then_some(()), move |(), _| scrim(&this))
}

fn scrim(layer: &Rc<ProfileLayer>) -> View {
	let close = layer.clone();
	let window = layer.window.clone();
	div()
		.absolute()
		.left(0.)
		.top(0.)
		.items_center()
		.bg(tok("overlay"))
		.reactive_style(move || {
			let (width, height) = window.get();
			Styles::new().w(width).h(height).pt(height * TOP_SHARE)
		})
		.on_event(move |event| {
			if event.phase == EventPhase::Target && matches!(event.event, InputEvent::PointerDown { .. }) {
				close.custom.close();
				close.picker.close();
				event.stop_propagation();
			}
		})
		.child(panel(layer))
}

fn panel(layer: &Rc<ProfileLayer>) -> View {
	let form = layer.custom.clone();
	let (clear, cancel, save) = (layer.clone(), layer.clone(), layer.clone());
	column()
		.w(WIDTH)
		.p(16.)
		.gap(16.)
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok("divider"))
		.relative()
		.bg(tok("background_primary"))
		.text_color(tok("text_normal"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.child(close_button(layer))
		.child(
			column()
				.gap(4.)
				.child(
					label("Set a custom status", "font_md")
						.font_weight(600)
						.text_color(tok("header_primary")),
				)
				.child(parts::section_label("What's cookin', good lookin'?"))
				.child(
					row()
						.mt(4.)
						.items_center()
						.gap(8.)
						.child(emoji_button(layer))
						.child(
							text_input("Custom status", form.text.clone())
								.placeholder("Support has arrived!")
								.grow()
								.min_w(0.)
								.h(INPUT)
								.px(12.)
								.rounded(tok("radius_sm"))
								.border(1.)
								.border_color(tok("button_secondary"))
								.bg(tok("input_background"))
								.text_size(tok("font_sm"))
								.text_color(tok("text_normal")),
						),
				),
		)
		.child(
			column().gap(8.).child(parts::section_label("Clear after")).child(
				row().flex_wrap().gap(8.).children(
					ClearAfter::ALL
						.into_iter()
						.map(|choice| clear_chip(&form.clear_after, choice))
						.collect::<Vec<_>>(),
				),
			),
		)
		.child(
			row()
				.justify_end()
				.gap(8.)
				.child(footer_button("Clear Status", None, move || {
					clear.request(Request::Command(Command::SetCustomStatus {
						text: None,
						emoji: None,
						expires: None,
					}));
					clear.custom.close();
				}))
				.child(footer_button("Cancel", Some("button_secondary"), move || {
					cancel.custom.close();
					cancel.picker.close();
				}))
				.child(footer_button("Save", Some("brand"), move || {
					save.request(Request::Command(save.custom.command(Utc::now())));
					save.custom.close();
					save.picker.close();
				})),
		)
}

fn emoji_button(layer: &Rc<ProfileLayer>) -> View {
	let emoji = layer.custom.emoji.clone();
	let picker = layer.clone();
	button()
		.size(FIELD, FIELD)
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(tok("input_background"))
		.hover(|style| style.bg(tok("modifier_selected")))
		.cursor(Cursor::Pointer)
		.on_event(move |event| {
			if let InputEvent::PointerDown { x, y, .. } = event.event {
				picker.custom.spot.set((x, y));
			}
		})
		.on_click({
			let layer = layer.clone();
			move || layer.request(Request::Picker)
		})
		.child(switch(
			move || emoji.get(),
			|glyph: Option<String>, _| match glyph {
				Some(glyph) => emoji::glyph(glyph, STATUS_GLYPH),
				None => icon("emoji", 24., "interactive_normal"),
			},
		))
}

fn clear_chip(selected: &Signal<ClearAfter>, choice: ClearAfter) -> View {
	let (chosen, pick) = (selected.clone(), selected.clone());
	button()
		.h(CHIP)
		.px(12.)
		.border(0.)
		.items_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if chosen.get() == choice {
				Styles::new().bg(tok("brand")).text_color(tok("text_on_brand"))
			} else {
				Styles::new()
					.bg(tok("background_tertiary"))
					.text_color(tok("interactive_normal"))
			}
		})
		.on_click(move || {
			pick.set(choice);
		})
		.child(label(choice.label(), "font_sm").font_weight(500))
}

fn footer_button(caption: &str, fill: Option<&'static str>, action: impl FnMut() + 'static) -> View {
	let color = match fill {
		Some("brand") => "text_on_brand",
		_ => "text_normal",
	};
	button()
		.h(BUTTON)
		.px(16.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.bg(match fill {
			Some(fill) => Themed::Token(tok(fill)),
			None => Themed::Value(Color(0, 0, 0, 0)),
		})
		.text_size(tok("font_sm"))
		.font_weight(500)
		.text_color(tok(color))
		.child(text(caption.to_owned()))
		.on_click(action)
}

fn close_button(layer: &Rc<ProfileLayer>) -> View {
	let close = layer.clone();
	button()
		.absolute()
		.top(12.)
		.right(12.)
		.size(CLOSE + 8., CLOSE + 8.)
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(Themed::Value(Color(0, 0, 0, 0)))
		.hover(|style| style.bg(tok("modifier_hover")))
		.cursor(Cursor::Pointer)
		.child(icon("close", CLOSE, "interactive_normal"))
		.on_click(move || {
			close.custom.close();
			close.picker.close();
		})
}
