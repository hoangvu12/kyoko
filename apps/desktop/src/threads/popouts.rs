use std::rc::Rc;

use bridge::{PinEntry, ThreadEntry, ThreadTab};
use media::ImageName;
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::style::ObjectFit;
use zgx::theme::Themed;

use super::{PinsShown, Request, ThreadsLayer, ThreadsShown};
use crate::timeline::{TypeSize, icon, tok};

const HEADER: f32 = 48.;
const GLYPH: f32 = 24.;
const PORTRAIT: f32 = 32.;
const EMPTY_GLYPH: f32 = 48.;
const SMALL_BUTTON: f32 = 24.;
const BUTTON: f32 = 32.;
const PILL: f32 = 32.;
const CLOSE_HIT: f32 = 28.;
const CLOSE_GLYPH: f32 = 20.;
const PIN_LINES: u32 = 5;
const NO_THREADS: &str = "No threads yet. Start one from any message.";
const NO_PINS: &str = "This channel has no pinned messages yet.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
	Primary,
	Outline,
	Ghost,
}

pub fn clear() -> Themed<Color> {
	Themed::Value(Color(0, 0, 0, 0))
}

fn button_with(label: &str, fill: Fill, height: f32, padding: f32) -> View {
	let base = button()
		.h(height)
		.px(padding)
		.py(0.)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.text_size(tok("font_sm"))
		.font_weight(500)
		.child(text(label.to_owned()));
	match fill {
		Fill::Primary => base
			.border(0.)
			.bg(tok("brand"))
			.text_color(tok("text_on_brand"))
			.hover(|style| style.bg(tok("brand_hover"))),
		Fill::Outline => base
			.border(1.)
			.border_color(tok("divider"))
			.bg(clear())
			.text_color(tok("text_normal"))
			.hover(|style| style.bg(tok("modifier_hover"))),
		Fill::Ghost => base
			.border(0.)
			.bg(clear())
			.text_color(tok("text_normal"))
			.hover(|style| style.bg(tok("modifier_hover"))),
	}
}

pub fn small_button(label: &str, fill: Fill) -> View {
	button_with(label, fill, SMALL_BUTTON, 8.)
}

pub fn wide_button(label: &str, fill: Fill) -> View {
	button_with(label, fill, BUTTON, 12.)
}

pub fn close_button(on_click: impl FnMut() + 'static) -> View {
	button()
		.size(CLOSE_HIT, CLOSE_HIT)
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(clear())
		.cursor(Cursor::Pointer)
		.child(icon("close", CLOSE_GLYPH, "interactive_normal"))
		.on_click(on_click)
		.tooltip("Close")
}

fn header(glyph: &'static str, title: &str) -> View {
	row()
		.w_full()
		.h(HEADER)
		.shrink_0()
		.items_center()
		.gap(8.)
		.px(16.)
		.border_edges(zgx::scene::Insets {
			bottom: 1.,
			..Default::default()
		})
		.border_color(tok("divider"))
		.child(icon(glyph, GLYPH, "text_muted"))
		.child(
			text(title.to_owned())
				.grow()
				.min_w(0.)
				.truncate()
				.type_size("font_md")
				.font_weight(600)
				.text_color(tok("header_primary")),
		)
}

fn empty(glyph: &'static str, message: &str) -> View {
	column()
		.w_full()
		.items_center()
		.gap(8.)
		.p(32.)
		.child(icon(glyph, EMPTY_GLYPH, "interactive_muted"))
		.child(
			text(message.to_owned())
				.text_wrap(true)
				.type_size("font_sm")
				.text_color(tok("text_muted")),
		)
}

fn frame(background: &str) -> View {
	column()
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok(background))
		.text_color(tok("text_normal"))
		.overflow_hidden()
}

pub fn threads(layer: &Rc<ThreadsLayer>, channel_id: Snowflake) -> View {
	let shown = layer.threads.clone();
	let tab = layer.tab.clone();
	let (create, close) = (layer.clone(), layer.clone());
	let creatable = shown.clone();
	let list_layer = layer.clone();
	frame("background_floating")
		.child(
			header("thread", "Threads")
				.child(div().child_when(
					move || creatable.with(|shown| shown.can_create).then_some(()),
					move |(), _| {
						let create = create.clone();
						small_button("Create", Fill::Primary)
							.on_click(move || create.request(Request::NewThread(channel_id)))
					},
				))
				.child(close_button(move || close.close_popout())),
		)
		.child(
			row()
				.w_full()
				.shrink_0()
				.gap(8.)
				.px(16.)
				.py(8.)
				.child(pill(layer, &tab, ThreadTab::Active, "Active", channel_id))
				.child(pill(layer, &tab, ThreadTab::Archived, "Archived", channel_id)),
		)
		.child(div().grow().min_h(0.).w_full().overflow_hidden().child(switch(
			move || shown.get(),
			move |shown: Rc<ThreadsShown>, _| thread_rows(&list_layer, &shown),
		)))
}

fn pill(
	layer: &Rc<ThreadsLayer>,
	tab: &Signal<ThreadTab>,
	choice: ThreadTab,
	label: &str,
	channel_id: Snowflake,
) -> View {
	let (current, pick, layer) = (tab.clone(), tab.clone(), layer.clone());
	button()
		.h(PILL)
		.px(12.)
		.py(0.)
		.items_center()
		.rounded(tok("radius_sm"))
		.border(1.)
		.cursor(Cursor::Pointer)
		.text_size(tok("font_sm"))
		.font_weight(500)
		.reactive_style(move || {
			if current.get() == choice {
				Styles::new()
					.border_color(tok("brand"))
					.bg(tok("brand_tint"))
					.text_color(tok("interactive_active"))
			} else {
				Styles::new()
					.border_color(tok("divider"))
					.bg(tok("background_secondary"))
					.text_color(tok("interactive_normal"))
			}
		})
		.on_click(move || {
			pick.set(choice);
			if choice == ThreadTab::Archived {
				layer.request(Request::Archived(channel_id));
			}
		})
		.child(text(label.to_owned()))
}

fn thread_rows(layer: &Rc<ThreadsLayer>, shown: &ThreadsShown) -> View {
	if shown.entries.is_empty() {
		return empty("thread", NO_THREADS);
	}
	column()
		.w_full()
		.children(shown.entries.iter().map(|entry| thread_row(layer, entry)))
}

fn thread_row(layer: &Rc<ThreadsLayer>, entry: &ThreadEntry) -> View {
	let (open, member) = (layer.clone(), layer.clone());
	let (id, joined) = (entry.id, entry.joined);
	row()
		.mx(8.)
		.px(12.)
		.py(8.)
		.gap(12.)
		.items_center()
		.rounded(tok("radius_md"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("modifier_hover")))
		.on_click(move || open.request(Request::OpenThread(id)))
		.child(icon(
			if entry.archived { "archive" } else { "thread" },
			GLYPH,
			"interactive_normal",
		))
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					text(entry.name.clone())
						.truncate()
						.type_size("font_md")
						.font_weight(600)
						.text_color(tok("header_primary")),
				)
				.child(
					text(entry.detail.clone())
						.truncate()
						.type_size("font_sm")
						.text_color(tok("text_muted")),
				),
		)
		.child(
			small_button(if joined { "Leave Thread" } else { "Join Thread" }, Fill::Outline).on_click(
				move || {
					let command = if joined {
						Command::LeaveThread { thread_id: id }
					} else {
						Command::JoinThread { thread_id: id }
					};
					member.request(Request::Command(command));
				},
			),
		)
}

pub fn pins(layer: &Rc<ThreadsLayer>, channel_id: Snowflake) -> View {
	let shown = layer.pins.clone();
	let close = layer.clone();
	let list_layer = layer.clone();
	frame("background_secondary")
		.child(header("pin", "Pinned Messages").child(close_button(move || close.close_popout())))
		.child(
			div()
				.grow()
				.min_h(0.)
				.w_full()
				.pt(8.)
				.overflow_hidden()
				.child(switch(
					move || shown.get(),
					move |shown: Rc<PinsShown>, _| pin_cards(&list_layer, channel_id, &shown),
				)),
		)
}

fn pin_cards(layer: &Rc<ThreadsLayer>, channel_id: Snowflake, shown: &PinsShown) -> View {
	if shown.loaded && shown.entries.is_empty() {
		return empty("pin", NO_PINS);
	}
	column().w_full().children(
		shown
			.entries
			.iter()
			.map(|(entry, avatar)| pin_card(layer, channel_id, entry, avatar.as_ref(), shown.can_manage)),
	)
}

fn pin_card(
	layer: &Rc<ThreadsLayer>,
	channel_id: Snowflake,
	entry: &PinEntry,
	avatar: Option<&ImageName>,
	can_manage: bool,
) -> View {
	let (jump, unpin) = (layer.clone(), layer.clone());
	let message_id = entry.id;
	column()
		.mx(8.)
		.mb(8.)
		.p(12.)
		.gap(8.)
		.rounded(tok("radius_md"))
		.bg(tok("background_primary"))
		.border(1.)
		.border_color(tok("divider"))
		.child(
			row()
				.items_center()
				.gap(8.)
				.child(portrait(layer, avatar, &entry.author))
				.child(
					column()
						.grow()
						.min_w(0.)
						.child(
							text(entry.author.clone())
								.truncate()
								.type_size("font_sm")
								.font_weight(600)
								.text_color(tok("header_primary")),
						)
						.child(
							text(entry.time.clone())
								.type_size("font_tiny")
								.text_color(tok("text_muted")),
						),
				)
				.child(small_button("Jump", Fill::Outline).on_click(move || {
					jump.request(Request::Jump {
						channel_id,
						message_id,
					})
				}))
				.when(can_manage, move |row| {
					row.child(
						button()
							.size(CLOSE_HIT, CLOSE_HIT)
							.shrink_0()
							.p(0.)
							.border(0.)
							.items_center()
							.justify_center()
							.rounded(tok("radius_sm"))
							.bg(clear())
							.cursor(Cursor::Pointer)
							.child(icon("unpin", CLOSE_GLYPH, "interactive_normal"))
							.tooltip("Unpin Message")
							.on_click(move || {
								unpin.request(Request::Unpin {
									channel_id,
									message_id,
								})
							}),
					)
				}),
		)
		.child(
			text(entry.text.clone())
				.text_wrap(true)
				.line_clamp(PIN_LINES)
				.type_size("font_sm")
				.text_color(tok("text_normal")),
		)
}

fn portrait(layer: &Rc<ThreadsLayer>, avatar: Option<&ImageName>, name: &str) -> View {
	let initial = name.chars().next().map(String::from).unwrap_or_default();
	let base = div()
		.size(PORTRAIT, PORTRAIT)
		.shrink_0()
		.rounded(PORTRAIT / 2.)
		.overflow_hidden()
		.bg(tok("brand"))
		.items_center()
		.justify_center();
	let Some(avatar) = avatar else {
		return base.child(
			text(initial)
				.text_size(PORTRAIT * 0.42)
				.font_weight(600)
				.text_color(tok("text_on_brand")),
		);
	};
	let picture = layer.pictures.picture(avatar);
	base.reactive_style(move || match picture.get() {
		Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
		None => Styles::new(),
	})
}
