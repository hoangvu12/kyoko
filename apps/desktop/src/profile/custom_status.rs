use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{StatusChoice, StatusLine};
use chrono::{DateTime, Duration, Local, TimeDelta, Utc};
use media::ImageName;
use session::Command;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::{Runtime, Signal};

use super::parts::{self, words};
use super::{ProfileLayer, Request};
use crate::emoji;
use crate::kit::{self, Variant};
use crate::moderation::kit::select;
use crate::timeline::icons::{glyph, icon};
use crate::timeline::tok;

const MAX_LENGTH: usize = 128;
const FIELD_GLYPH: f32 = 20.;
const BUBBLE_GLYPH: f32 = 16.;
const CLEAR_GLYPH: f32 = 16.;
const STATUS_LABELS: [&str; 4] = ["Online", "Idle", "Do Not Disturb", "Invisible"];

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

	pub const LABELS: [&'static str; 5] = ["Today", "4 hours", "1 hour", "30 minutes", "Don't clear"];

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
	clear_after: Signal<usize>,
	clear_open: Signal<bool>,
	status: Signal<usize>,
	status_open: Signal<bool>,
	initial_status: Cell<usize>,
	name: RefCell<String>,
	avatar: RefCell<Option<ImageName>>,
	spot: Cell<(f32, f32)>,
}

impl CustomStatusForm {
	pub fn new(runtime: &Runtime) -> Self {
		Self {
			open: runtime.signal(false),
			text: runtime.signal(String::new()),
			emoji: runtime.signal(None),
			clear_after: runtime.signal(0),
			clear_open: runtime.signal(false),
			status: runtime.signal(0),
			status_open: runtime.signal(false),
			initial_status: Cell::new(0),
			name: RefCell::new(String::new()),
			avatar: RefCell::new(None),
			spot: Cell::new((0., 0.)),
		}
	}

	pub fn open(&self, current: StatusLine, status: StatusChoice, name: String, avatar: Option<ImageName>) {
		self.text.set(current.text.unwrap_or_default());
		self.emoji
			.set(current.emoji_id.is_none().then_some(current.emoji_name).flatten());
		self.clear_after.set(0);
		let index = StatusChoice::ALL
			.iter()
			.position(|choice| *choice == status)
			.unwrap_or(0);
		self.status.set(index);
		self.initial_status.set(index);
		*self.name.borrow_mut() = name;
		*self.avatar.borrow_mut() = avatar;
		self.open.set(true);
	}

	pub fn close(&self) {
		self.clear_open.set(false);
		self.status_open.set(false);
		self.open.set(false);
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn open_signal(&self) -> Signal<bool> {
		self.open.clone()
	}

	pub fn avatar(&self) -> Option<ImageName> {
		self.is_open().then(|| self.avatar.borrow().clone()).flatten()
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
		let clear_after = self
			.clear_after
			.with_untracked(|index| ClearAfter::ALL.get(*index).copied())
			.unwrap_or(ClearAfter::Today);
		Command::SetCustomStatus {
			text,
			emoji,
			expires: content.then(|| clear_after.expires(now)).flatten(),
		}
	}

	pub fn status_change(&self) -> Option<Command> {
		let index = self.status.with_untracked(|index| *index);
		(index != self.initial_status.get())
			.then(|| StatusChoice::ALL.get(index).copied())
			.flatten()
			.map(|choice| Command::SetStatus(choice.kind()))
	}
}

pub fn dialog(layer: &Rc<ProfileLayer>) -> View {
	let form = layer.custom.clone();
	let open = form.open.clone();
	let this = layer.clone();
	let tracked = crate::scrim::track(&form.open);
	div().child_when(
		move || open.get().then_some(()),
		move |(), _| {
			let _ = &tracked;
			scrim(&this)
		},
	)
}

fn scrim(layer: &Rc<ProfileLayer>) -> View {
	let close = layer.clone();
	let window = layer.window.clone();
	div()
		.absolute()
		.left(0.)
		.top(0.)
		.items_center()
		.justify_center()
		.reactive_style(move || {
			let (width, height) = window.get();
			Styles::new().w(width).h(height)
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
	let (close, cancel, save) = (layer.clone(), layer.clone(), layer.clone());
	let question = format!("What's cookin', {}?", form.name.borrow());
	column()
		.id("custom-status")
		.w(tok("modal_width_sm"))
		.overflow_hidden()
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_4"))
		.text_color(tok("text_3"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.child(kit::header(kit::title("Set a custom status"), None, move || {
			close.custom.close();
			close.picker.close();
		}))
		.child(
			kit::body()
				.child(preview(layer))
				.child(kit::field(&question, status_field(layer)))
				.child(kit::field(
					"Clear after",
					select(
						"Clear after",
						&ClearAfter::LABELS,
						&form.clear_after,
						form.clear_open.clone(),
					),
				))
				.child(kit::field(
					"Status",
					select("Status", &STATUS_LABELS, &form.status, form.status_open.clone()),
				))
				.child(kit::footer(vec![
					kit::action("Cancel", Variant::Link, move || {
						cancel.custom.close();
						cancel.picker.close();
					}),
					kit::action("Save", Variant::Primary, move || {
						save.request(Request::Command(save.custom.command(Utc::now())));
						if let Some(change) = save.custom.status_change() {
							save.request(Request::Command(change));
						}
						save.custom.close();
						save.picker.close();
					})
					.id("custom-status-save"),
				])),
		)
}

fn preview(layer: &Rc<ProfileLayer>) -> View {
	let form = layer.custom.clone();
	let (text_signal, emoji_signal) = (form.text.clone(), form.emoji.clone());
	let avatar = form.avatar.borrow().clone();
	row()
		.w_full()
		.items_start()
		.gap(tok("space_12"))
		.child(parts::avatar(
			&layer.pictures,
			avatar.as_ref(),
			parts::size(&layer.runtime, "avatar_size_lg"),
			None,
			"bg_4",
		))
		.child(div().min_w(0.).child_when(
			move || {
				let text = text_signal.get();
				let emoji = emoji_signal.get();
				(!text.trim().is_empty() || emoji.is_some()).then_some((text, emoji))
			},
			|(value, glyph_text): (String, Option<String>), _| {
				row()
					.mt(tok("space_8"))
					.items_center()
					.gap(tok("space_4"))
					.min_w(0.)
					.px(tok("space_12"))
					.py(tok("space_8"))
					.rounded(tok("radius_md"))
					.bg(tok("bg_3"))
					.border(tok("border_thickness"))
					.border_color(tok("border"))
					.when_some(glyph_text, |bubble, glyph_text| {
						bubble.child(emoji::glyph(glyph_text, BUBBLE_GLYPH))
					})
					.child(words(value, "sm").min_w(0.).truncate().text_color(tok("text_3")))
			},
		))
}

fn status_field(layer: &Rc<ProfileLayer>) -> View {
	let form = layer.custom.clone();
	let (text_value, emoji_value) = (form.text.clone(), form.emoji.clone());
	let (clear_text, clear_emoji) = (form.text.clone(), form.emoji.clone());
	row()
		.w_full()
		.h(tok("input_height"))
		.items_center()
		.gap(tok("space_8"))
		.px(tok("space_12"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.hover(|style| style.border_color(tok("border")))
		.child(emoji_button(layer))
		.child(
			text_input("Custom status".to_owned(), form.text.clone())
				.id("custom-status-text")
				.placeholder("Support has arrived!".to_owned())
				.w(0.)
				.grow()
				.min_w(0.)
				.h_full()
				.p(0.)
				.border(0.)
				.bg(kit::clear())
				.text_color(tok("text_3"))
				.text_size(tok("font_size_md"))
				.focus(|style| style.outline_width(0.)),
		)
		.child(div().child_when(
			move || (!text_value.get().is_empty() || emoji_value.get().is_some()).then_some(()),
			move |(), _| {
				let (text, emoji) = (clear_text.clone(), clear_emoji.clone());
				button()
					.id("custom-status-clear")
					.focusable(false)
					.w(tok("icon_size_md"))
					.h(tok("icon_size_md"))
					.p(0.)
					.border(0.)
					.items_center()
					.justify_center()
					.bg(kit::clear())
					.cursor(Cursor::Pointer)
					.text_color(tok("text_4"))
					.hover(|style| style.text_color(tok("text_3")))
					.child(glyph("close", CLEAR_GLYPH))
					.on_click(move || {
						text.set(String::new());
						emoji.set(None);
					})
					.tooltip("Clear")
			},
		))
}

fn emoji_button(layer: &Rc<ProfileLayer>) -> View {
	let emoji = layer.custom.emoji.clone();
	let picker = layer.clone();
	button()
		.id("custom-status-emoji")
		.focusable(false)
		.w(tok("icon_size_md"))
		.h(tok("icon_size_md"))
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.bg(kit::clear())
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
			|glyph_text: Option<String>, _| match glyph_text {
				Some(glyph_text) => emoji::glyph(glyph_text, FIELD_GLYPH),
				None => icon("emoji", FIELD_GLYPH, "text_4"),
			},
		))
}
