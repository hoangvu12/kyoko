use std::rc::Rc;

use bridge::{PinEntry, ThreadEntry, ThreadSection};
use media::ImageName;
use protocol::Snowflake;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::Runtime;
use zgx::scene::{Color, Insets};
use zgx::theme::Themed;

use super::{PinsShown, Request, ThreadsLayer, ThreadsShown};
use crate::kit::{self, Variant};
use crate::profile::parts;
use crate::timeline::{Pictures, icon, tok};

const GLYPH: f32 = 20.;
const ROW_AVATAR: f32 = 16.;
const CARD_AVATAR: f32 = 40.;
const EMPTY_GLYPH: f32 = 48.;
const SEARCH_GLYPH: f32 = 16.;
const COUNT_GLYPH: f32 = 16.;
const CARD_BUTTON: f32 = 24.;
const PIN_LINES: u32 = 5;
const NO_THREADS: &str = "No threads yet. Start one from any message.";
const NO_MATCH: &str = "No threads match your search.";
const NO_PINS: &str = "This channel has no pinned messages yet.";
const SEARCH_PLACEHOLDER: &str = "Search For Thread";

pub fn clear() -> Themed<Color> {
	Themed::Value(Color(0, 0, 0, 0))
}

pub fn close_button(on_click: impl FnMut() + 'static) -> View {
	button()
		.w(tok("button_size"))
		.h(tok("button_size"))
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(clear())
		.cursor(Cursor::Pointer)
		.text_color(tok("text_4"))
		.hover(|style| style.text_color(tok("text_3")))
		.child(crate::timeline::icons::glyph("close", GLYPH))
		.on_click(on_click)
		.tooltip("Close")
}

fn words(value: impl Into<String>, scale: &str) -> View {
	parts::words(value, scale)
}

fn header(glyph: &'static str, title: &str) -> View {
	row()
		.w_full()
		.h(tok("button_height_lg"))
		.shrink_0()
		.items_center()
		.gap(tok("space_8"))
		.pl(tok("space_16"))
		.pr(tok("space_12"))
		.border_edges(Insets {
			bottom: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.child(icon(glyph, GLYPH, "text_4"))
		.child(
			words(title.to_owned(), "md")
				.grow()
				.min_w(0.)
				.truncate()
				.font_weight(600)
				.text_color(tok("text_2")),
		)
}

fn empty(glyph: &'static str, message: &str) -> View {
	column()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.p(tok("space_32"))
		.child(icon(glyph, EMPTY_GLYPH, "text_5"))
		.child(
			words(message.to_owned(), "sm")
				.text_wrap(true)
				.text_color(tok("text_4")),
		)
}

fn frame() -> View {
	column()
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_3"))
		.text_color(tok("text_3"))
		.overflow_hidden()
}

fn search_field(layer: &Rc<ThreadsLayer>) -> View {
	row()
		.w(tok("thread_search_width"))
		.h(tok("button_height_sm"))
		.shrink_0()
		.items_center()
		.gap(tok("space_8"))
		.px(tok("space_8"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.child(
			text_input(SEARCH_PLACEHOLDER, layer.search.clone())
				.placeholder(SEARCH_PLACEHOLDER)
				.grow()
				.min_w(0.)
				.h_full()
				.p(0.)
				.border(0.)
				.bg(clear())
				.text_color(tok("text_3"))
				.text_size(tok("font_size_sm"))
				.focus(|style| style.outline_width(0.)),
		)
		.child(icon("search", SEARCH_GLYPH, "text_4"))
}

fn create_button(on_click: impl FnMut() + 'static) -> View {
	kit::action("Create", Variant::Primary, on_click).h(tok("button_height_sm"))
}

pub fn threads(layer: &Rc<ThreadsLayer>, channel_id: Snowflake) -> View {
	let shown = layer.threads.clone();
	let query = layer.search.clone();
	let (create, close) = (layer.clone(), layer.clone());
	let creatable = shown.clone();
	let list_layer = layer.clone();
	frame()
		.max_h(parts::size(&layer.runtime, "thread_popout_height"))
		.child(
			header("thread", "Threads")
				.child(search_field(layer))
				.child(div().child_when(
					move || creatable.with(|shown| shown.can_create).then_some(()),
					move |(), _| {
						let create = create.clone();
						create_button(move || create.request(Request::NewThread(channel_id)))
					},
				))
				.child(close_button(move || close.close_popout())),
		)
		.child(
			scroll(layer.runtime.signal(0.))
				.grow()
				.min_h(0.)
				.w_full()
				.child(switch(
					move || (shown.get(), query.get().trim().to_lowercase()),
					move |(shown, query): (Rc<ThreadsShown>, String), _| {
						thread_sections(&list_layer, &shown, &query)
					},
				)),
		)
}

fn thread_sections(layer: &Rc<ThreadsLayer>, shown: &ThreadsShown, query: &str) -> View {
	let sections: Vec<ThreadSection> = shown
		.sections
		.iter()
		.map(|section| ThreadSection {
			title: section.title.clone(),
			entries: section
				.entries
				.iter()
				.filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(query))
				.cloned()
				.collect(),
		})
		.filter(|section| !section.entries.is_empty())
		.collect();
	if sections.is_empty() {
		return empty("thread", if query.is_empty() { NO_THREADS } else { NO_MATCH });
	}
	column()
		.w_full()
		.px(tok("space_12"))
		.pb(tok("space_12"))
		.gap(tok("space_8"))
		.children(sections.iter().flat_map(|section| {
			std::iter::once(section_title(&section.title))
				.chain(
					section
						.entries
						.iter()
						.map(|entry| thread_row(layer, shown, entry)),
				)
				.collect::<Vec<_>>()
		}))
}

fn section_title(title: &str) -> View {
	words(title.to_uppercase(), "xs")
		.pt(tok("space_12"))
		.px(tok("space_4"))
		.pb(tok("space_4"))
		.font_weight(700)
		.text_color(tok("text_4"))
}

fn thread_row(layer: &Rc<ThreadsLayer>, shown: &ThreadsShown, entry: &ThreadEntry) -> View {
	let open = layer.clone();
	let id = entry.id;
	let avatar = entry.author_id.and_then(|user_id| {
		shown
			.avatars
			.iter()
			.find(|(owner, _)| *owner == user_id)
			.map(|(_, name)| name.clone())
	});
	let meta = row()
		.w_full()
		.mt(tok("space_4"))
		.items_center()
		.gap(tok("space_6"))
		.child(portrait(layer, avatar.as_ref(), ROW_AVATAR))
		.child(
			words(entry.author.clone(), "sm")
				.shrink_0()
				.font_weight(500)
				.text_color(tok("text_2")),
		)
		.when(!entry.snippet.is_empty(), |meta| {
			meta.child(
				words(entry.snippet.clone(), "sm")
					.min_w(0.)
					.truncate()
					.text_color(tok("text_4")),
			)
		})
		.child(
			words(format!("\u{2022} {}", entry.activity), "sm")
				.shrink_0()
				.text_color(tok("text_4")),
		);
	row()
		.w_full()
		.px(tok("space_16"))
		.py(tok("space_12"))
		.gap(tok("space_12"))
		.items_center()
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("hover")))
		.on_click(move || open.request(Request::OpenThread(id)))
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					words(entry.name.clone(), "md")
						.truncate()
						.font_weight(600)
						.text_color(tok(if entry.archived { "text_4" } else { "text_2" })),
				)
				.child(meta),
		)
		.child(
			row()
				.shrink_0()
				.items_center()
				.gap(tok("space_4"))
				.child(icon("chat", COUNT_GLYPH, "text_4"))
				.child(
					words(entry.count.clone(), "sm")
						.font_weight(600)
						.text_color(tok("text_4")),
				),
		)
}

pub fn pins(layer: &Rc<ThreadsLayer>, channel_id: Snowflake) -> View {
	let shown = layer.pins.clone();
	let close = layer.clone();
	let list_layer = layer.clone();
	frame()
		.h(tok("pins_popout_height"))
		.child(header("pin", "Pinned Messages").child(close_button(move || close.close_popout())))
		.child(
			scroll(layer.runtime.signal(0.))
				.grow()
				.min_h(0.)
				.w_full()
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
	column().w_full().p(tok("space_12")).gap(tok("space_8")).children(
		shown
			.entries
			.iter()
			.map(|(entry, avatar)| pin_card(layer, channel_id, entry, avatar.as_ref(), shown.can_pin)),
	)
}

pub struct CardHead<'a> {
	pub avatar: Option<&'a ImageName>,
	pub author: &'a str,
	pub author_color: Themed<Color>,
	pub time: &'a str,
}

pub fn message_card(
	runtime: &Runtime,
	pictures: &Rc<Pictures>,
	head: CardHead<'_>,
	body: View,
	tools: View,
) -> View {
	let hovered = runtime.signal(false);
	let (enter, leave, shown, border) = (hovered.clone(), hovered.clone(), hovered.clone(), hovered);
	div()
		.relative()
		.w_full()
		.pl(tok("message_card_inset"))
		.pr(tok("space_12"))
		.py(tok("space_8"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.reactive_style(move || {
			Styles::new().border_color(tok(if border.get() { "border_light" } else { "border" }))
		})
		.on_event(move |event| match event.event {
			InputEvent::PointerEnter => {
				enter.set(true);
			}
			InputEvent::PointerLeave => {
				leave.set(false);
			}
			_ => {}
		})
		.child(div().absolute().left(12.).top(8.).child(parts::avatar(
			pictures,
			head.avatar,
			CARD_AVATAR,
			None,
			"bg_4",
		)))
		.child(
			row()
				.w_full()
				.items_end()
				.gap(tok("space_6"))
				.child(
					text(head.author.to_owned())
						.shrink_0()
						.text_size(tok("font_size_md"))
						.line_height(tok("line_height_md"))
						.font_weight(500)
						.text_color(head.author_color),
				)
				.child(
					text(head.time.to_owned())
						.min_w(0.)
						.truncate()
						.text_size(tok("font_size_xs"))
						.line_height(tok("line_height_md"))
						.font_weight(500)
						.text_color(tok("text_5")),
				),
		)
		.child(body)
		.child(tools.absolute().top(8.).right(8.).reactive_style(move || {
			if shown.get() {
				Styles::new().visible()
			} else {
				Styles::new().invisible()
			}
		}))
}

pub fn jump_button(on_click: impl FnMut() + 'static) -> View {
	button()
		.h(CARD_BUTTON)
		.px(tok("space_6"))
		.py(0.)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(tok("bg_2"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.cursor(Cursor::Pointer)
		.text_color(tok("text_3"))
		.text_size(tok("font_size_xs"))
		.line_height(tok("line_height_label"))
		.font_weight(500)
		.hover(|style| style.bg(tok("bg_1")))
		.child(text("Jump"))
		.on_click(on_click)
}

fn pin_card(
	layer: &Rc<ThreadsLayer>,
	channel_id: Snowflake,
	entry: &PinEntry,
	avatar: Option<&ImageName>,
	can_pin: bool,
) -> View {
	let (jump, unpin) = (layer.clone(), layer.clone());
	let message_id = entry.id;
	let tools = row()
		.gap(tok("space_4"))
		.child(jump_button(move || {
			jump.request(Request::Jump {
				channel_id,
				message_id,
			})
		}))
		.when(can_pin, move |tools| {
			tools.child(
				button()
					.size(CARD_BUTTON, CARD_BUTTON)
					.shrink_0()
					.p(0.)
					.border(0.)
					.items_center()
					.justify_center()
					.rounded(tok("radius_sm"))
					.bg(clear())
					.cursor(Cursor::Pointer)
					.text_color(tok("text_4"))
					.hover(|style| style.text_color(tok("text_3")))
					.child(crate::timeline::icons::glyph("close", SEARCH_GLYPH))
					.on_click(move || {
						unpin.request(Request::Unpin {
							channel_id,
							message_id,
						})
					})
					.tooltip("Unpin Message"),
			)
		});
	let body = text(entry.text.clone())
		.w_full()
		.text_wrap(true)
		.line_clamp(PIN_LINES)
		.text_size(tok("font_size_md"))
		.line_height(tok("line_height_md"))
		.text_color(tok("text_3"));
	let head = CardHead {
		avatar,
		author: &entry.author,
		author_color: Themed::Token(tok("text_2")),
		time: &entry.time,
	};
	message_card(&layer.runtime, &layer.pictures, head, body, tools)
}

fn portrait(layer: &Rc<ThreadsLayer>, avatar: Option<&ImageName>, side: f32) -> View {
	parts::avatar(&layer.pictures, avatar, side, None, "bg_4")
}
