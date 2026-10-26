use std::rc::Rc;

use bridge::{ForYouEntry, InboxTab, MentionCard, MentionGroup, Place, Span, UnreadGroup, UnreadRow};
use protocol::Snowflake;
use zgx::compose::prelude::*;
use zgx::scene::{Color, Insets};
use zgx::theme::Themed;

use super::{Filter, InboxLayer, InboxShown, Request};
use crate::kit::{self, Variant};
use crate::profile::parts;
use crate::threads::{CardHead, clear, jump_button, message_card};
use crate::timeline::{icon, tok};

const GLYPH: f32 = 20.;
const SMALL_GLYPH: f32 = 16.;
const EMPTY_GLYPH: f32 = 48.;
const CIRCLE: f32 = 24.;
const BADGE: f32 = 24.;
const AVATAR: f32 = 40.;
const MENTION_PAD: f32 = 2.;
const BADGE_MIN: f32 = 16.;
const NO_MENTIONS: &str = "Nobody has mentioned you in the last seven days.";
const LOADING: &str = "Loading your mentions\u{2026}";
const NO_UNREADS: &str = "You're all caught up. Unread channels show up here.";
const NO_FOR_YOU: &str = "Nothing new for you. Friend requests show up here.";

pub(super) fn popout(layer: &Rc<InboxLayer>) -> View {
	let filter_open = layer.filter_open.clone();
	let menu_layer = layer.clone();
	column()
		.relative()
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_3"))
		.text_color(tok("text_3"))
		.overflow_hidden()
		.child(header(layer))
		.child(body(layer))
		.child(div().child_when(
			move || filter_open.get().then_some(()),
			move |(), _| filter_menu(&menu_layer),
		))
}

fn words(value: impl Into<String>, scale: &str) -> View {
	parts::words(value, scale)
}

fn header(layer: &Rc<InboxLayer>) -> View {
	let tab = layer.tab.clone();
	let toggle = layer.clone();
	let title = row()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.child(icon("inbox", GLYPH, "text_4"))
		.child(
			text("Inbox")
				.grow()
				.min_w(0.)
				.text_size(tok("font_size_lg"))
				.line_height(tok("line_height_lg"))
				.font_weight(600)
				.text_color(tok("text_2")),
		)
		.child(
			square_button(
				"funnel",
				GLYPH,
				tok("button_size"),
				"Filter Mentions",
				move || {
					toggle.filter_open.update(|open| *open = !*open);
				},
			)
			.id("inbox-filter")
			.reactive_style(move || {
				if tab.get() == InboxTab::Mentions {
					Styles::new().visible()
				} else {
					Styles::new().invisible()
				}
			}),
		);
	column()
		.w_full()
		.shrink_0()
		.pt(tok("space_12"))
		.px(tok("space_16"))
		.border_edges(Insets {
			bottom: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.child(title)
		.child(
			row()
				.mt(tok("space_8"))
				.gap(tok("space_16"))
				.children(InboxTab::ALL.iter().map(|tab| tab_button(layer, *tab))),
		)
}

fn tab_button(layer: &Rc<InboxLayer>, tab: InboxTab) -> View {
	let (selected, select) = (layer.tab.clone(), layer.clone());
	div()
		.id(format!("inbox-tab-{}", tab_key(tab)))
		.cursor(Cursor::Pointer)
		.pt(tok("space_8"))
		.border_color(tok("accent_2"))
		.reactive_style(move || {
			if selected.get() == tab {
				Styles::new()
					.pb(6.)
					.border_edges(Insets {
						bottom: 2.,
						..Default::default()
					})
					.text_color(tok("text_2"))
			} else {
				Styles::new()
					.pb(tok("space_8"))
					.border_edges(Insets::default())
					.text_color(tok("text_4"))
			}
		})
		.on_click(move || select.select_tab(tab))
		.child(words(tab.label(), "sm").font_weight(500))
}

fn tab_key(tab: InboxTab) -> &'static str {
	match tab {
		InboxTab::ForYou => "for-you",
		InboxTab::Unreads => "unreads",
		InboxTab::Mentions => "mentions",
	}
}

fn square_button(
	glyph: &'static str,
	glyph_size: f32,
	side: impl Into<Themed<f32>> + Clone,
	tip: &'static str,
	on_click: impl FnMut() + 'static,
) -> View {
	button()
		.w(side.clone())
		.h(side)
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
		.child(crate::timeline::icons::glyph(glyph, glyph_size))
		.tooltip(tip)
		.on_click(on_click)
}

fn circle_button(glyph: &'static str, tip: &'static str, on_click: impl FnMut() + 'static) -> View {
	button()
		.size(CIRCLE, CIRCLE)
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(CIRCLE / 2.)
		.bg(tok("bg_4"))
		.cursor(Cursor::Pointer)
		.text_color(tok("text_4"))
		.hover(|style| style.text_color(tok("text_3")))
		.child(crate::timeline::icons::glyph(glyph, SMALL_GLYPH))
		.tooltip(tip)
		.on_click(on_click)
}

fn body(layer: &Rc<InboxLayer>) -> View {
	let (tab, shown) = (layer.tab.clone(), layer.shown.clone());
	let list = layer.clone();
	scroll(layer.runtime.signal(0.))
		.grow()
		.min_h(0.)
		.w_full()
		.child(switch(
			move || (tab.get(), shown.get()),
			move |(tab, shown): (InboxTab, Rc<InboxShown>), _| {
				let items = match tab {
					InboxTab::Mentions => mentions(&list, &shown),
					InboxTab::Unreads => unreads(&list, &shown),
					InboxTab::ForYou => for_you(&list, &shown),
				};
				column()
					.w_full()
					.pt(tok("space_4"))
					.px(tok("space_12"))
					.pb(tok("space_12"))
					.gap(tok("space_12"))
					.children(items)
			},
		))
}

fn empty(glyph: &'static str, message: &str) -> Vec<View> {
	vec![
		column()
			.id("inbox-empty")
			.w_full()
			.items_center()
			.gap(tok("space_8"))
			.p(tok("space_32"))
			.child(icon(glyph, EMPTY_GLYPH, "text_5"))
			.child(
				words(message.to_owned(), "sm")
					.text_wrap(true)
					.text_color(tok("text_4")),
			),
	]
}

fn guild_badge(
	layer: &Rc<InboxLayer>,
	shown: &InboxShown,
	guild_id: Option<Snowflake>,
	acronym: &str,
) -> View {
	let frame = row()
		.size(BADGE, BADGE)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.overflow_hidden();
	match shown.icon(guild_id) {
		Some(name) => parts::picture_box(
			&layer.pictures,
			Some(name),
			frame.bg(tok("bg_2")),
			ObjectFit::Cover,
		),
		None => frame.bg(tok("text_1")).child(
			text(acronym.to_owned())
				.text_size(tok("font_size_tag"))
				.font_weight(700)
				.text_color(tok("accent_5")),
		),
	}
}

fn place_header(layer: &Rc<InboxLayer>, shown: &InboxShown, place: &Place, close: View) -> View {
	row()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.pt(tok("space_8"))
		.px(tok("space_4"))
		.child(guild_badge(layer, shown, place.guild_id, &place.acronym))
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					row()
						.w_full()
						.items_center()
						.gap(tok("space_4"))
						.child(icon(place.channel_icon, SMALL_GLYPH, "text_4"))
						.child(
							words(place.channel.clone(), "md")
								.min_w(0.)
								.truncate()
								.font_weight(600)
								.text_color(tok("text_2")),
						),
				)
				.child(
					words(place.guild.clone(), "xs")
						.truncate()
						.text_color(tok("text_4")),
				),
		)
		.child(close)
}

fn mentions(layer: &Rc<InboxLayer>, shown: &InboxShown) -> Vec<View> {
	let Some(groups) = shown.mentions.as_ref() else {
		return empty("at_sign", LOADING);
	};
	if groups.is_empty() {
		return empty("at_sign", NO_MENTIONS);
	}
	groups
		.iter()
		.map(|group| mention_group(layer, shown, group))
		.collect()
}

fn mention_group(layer: &Rc<InboxLayer>, shown: &InboxShown, group: &MentionGroup) -> View {
	let remove = layer.clone();
	let ids: Vec<Snowflake> = group.cards.iter().map(|card| card.id).collect();
	let close = circle_button("close", "Remove from Inbox", move || {
		remove.request(Request::Remove(ids.clone()))
	})
	.id(format!("inbox-remove-{}", group.cards[0].id));
	column()
		.w_full()
		.gap(tok("space_8"))
		.child(place_header(layer, shown, &group.place, close))
		.children(
			group
				.cards
				.iter()
				.map(|card| mention_card(layer, shown, &group.place, card)),
		)
}

fn mention_card(layer: &Rc<InboxLayer>, shown: &InboxShown, place: &Place, card: &MentionCard) -> View {
	let jump = layer.clone();
	let (guild_id, channel_id, message_id) = (place.guild_id, place.channel_id, card.id);
	let tools = row().gap(tok("space_4")).child(
		jump_button(move || {
			jump.close();
			jump.request(Request::JumpTo {
				guild_id,
				channel_id,
				message_id,
			})
		})
		.id(format!("inbox-jump-{message_id}")),
	);
	let head = CardHead {
		avatar: shown.avatar(card.author_id),
		author: &card.author,
		author_color: match card.author_color {
			Some(color) => Themed::Value(rgb(color)),
			None => Themed::Token(tok("text_2")),
		},
		time: &card.time,
	};
	message_card(&layer.runtime, &layer.pictures, head, spans(&card.spans), tools)
		.id(format!("inbox-card-{message_id}"))
}

fn rgb(color: u32) -> Color {
	let [_, red, green, blue] = color.to_be_bytes();
	Color(red, green, blue, 255)
}

fn spans(spans: &[Span]) -> View {
	let runs = spans.iter().map(|span| {
		let run = text_span(span.text.clone());
		if span.mention {
			run.font_weight(500)
				.text_color(tok("mention_text"))
				.background(tok("mention_background"))
				.padding(MENTION_PAD, 0.)
		} else {
			run
		}
	});
	View::from(rich_text().children(runs))
		.w_full()
		.text_wrap(true)
		.text_size(tok("font_size_md"))
		.line_height(tok("line_height_md"))
		.text_color(tok("text_3"))
}

fn unreads(layer: &Rc<InboxLayer>, shown: &InboxShown) -> Vec<View> {
	if shown.unreads.is_empty() {
		return empty("mark_read", NO_UNREADS);
	}
	shown
		.unreads
		.iter()
		.map(|group| unread_group(layer, shown, group))
		.collect()
}

fn unread_group(layer: &Rc<InboxLayer>, shown: &InboxShown, group: &UnreadGroup) -> View {
	let read = layer.clone();
	let guild_id = group.guild_id;
	let mark = circle_button("mark_read", "Mark Server As Read", move || {
		read.request(Request::MarkGuildRead(guild_id))
	})
	.id(format!("inbox-read-guild-{guild_id}"));
	let header = row()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.pt(tok("space_8"))
		.px(tok("space_4"))
		.child(guild_badge(layer, shown, Some(guild_id), &group.acronym))
		.child(
			words(group.guild.clone(), "md")
				.grow()
				.min_w(0.)
				.truncate()
				.font_weight(600)
				.text_color(tok("text_2")),
		)
		.child(mark);
	column()
		.w_full()
		.gap(tok("space_8"))
		.child(header)
		.children(group.rows.iter().map(|row| unread_row(layer, row)))
}

fn unread_row(layer: &Rc<InboxLayer>, unread: &UnreadRow) -> View {
	let (open, jump, read) = (layer.clone(), layer.clone(), layer.clone());
	let (guild_id, channel_id) = (unread.place.guild_id, unread.place.channel_id);
	let badge = (unread.mentions > 0).then(|| {
		row()
			.h(tok("badge_size"))
			.min_w(BADGE_MIN)
			.px(tok("space_4"))
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("badge_size"))
			.bg(tok("accent_new"))
			.child(
				words(unread.mentions.to_string(), "xs")
					.font_weight(700)
					.text_color(tok("text_0")),
			)
	});
	row()
		.id(format!("inbox-unread-{channel_id}"))
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.px(tok("space_12"))
		.py(tok("space_8"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.border_color(tok("border_light")))
		.on_click(move || {
			open.close();
			open.request(Request::OpenChannel { guild_id, channel_id })
		})
		.child(icon(unread.place.channel_icon, GLYPH, "text_4"))
		.child(
			words(unread.place.channel.clone(), "md")
				.grow()
				.min_w(0.)
				.truncate()
				.font_weight(500)
				.text_color(tok("text_2")),
		)
		.when_some(badge, |row, badge| row.child(badge))
		.child(jump_button(move || {
			jump.close();
			jump.request(Request::OpenChannel { guild_id, channel_id })
		}))
		.child(
			square_button("mark_read", SMALL_GLYPH, CIRCLE, "Mark As Read", move || {
				read.request(Request::MarkRead(channel_id))
			})
			.id(format!("inbox-read-{channel_id}")),
		)
}

fn for_you(layer: &Rc<InboxLayer>, shown: &InboxShown) -> Vec<View> {
	if shown.for_you.is_empty() {
		return empty("friend_requests", NO_FOR_YOU);
	}
	shown
		.for_you
		.iter()
		.map(|entry| for_you_card(layer, shown, entry))
		.collect()
}

fn for_you_card(layer: &Rc<InboxLayer>, shown: &InboxShown, entry: &ForYouEntry) -> View {
	let (accept, ignore) = (layer.clone(), layer.clone());
	let user_id = entry.user_id;
	let line = View::from(
		rich_text().children([
			text_span(entry.name.clone())
				.font_weight(600)
				.text_color(tok("text_2")),
			text_span(format!(" {}", entry.text)),
		]),
	)
	.w_full()
	.text_wrap(true)
	.text_size(tok("font_size_md"))
	.line_height(tok("line_height_md"))
	.text_color(tok("text_3"));
	row()
		.id(format!("inbox-for-you-{user_id}"))
		.w_full()
		.items_center()
		.gap(tok("space_12"))
		.p(tok("space_12"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.child(parts::avatar(
			&layer.pictures,
			shown.avatar(user_id),
			AVATAR,
			None,
			"bg_4",
		))
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(line)
				.when(!entry.time.is_empty(), |column| {
					column.child(
						words(entry.time.clone(), "xs")
							.font_weight(500)
							.text_color(tok("text_5")),
					)
				}),
		)
		.child(
			kit::action("Accept", Variant::Primary, move || {
				accept.request(Request::Accept(user_id))
			})
			.h(tok("button_height_sm")),
		)
		.child(
			kit::action("Ignore", Variant::Secondary, move || {
				ignore.request(Request::Ignore(user_id))
			})
			.h(tok("button_height_sm")),
		)
}

fn filter_menu(layer: &Rc<InboxLayer>) -> View {
	let choice = |label: &str, key: &str, flag: fn(&mut Filter) -> &mut bool| {
		let (layer, current) = (layer.clone(), layer.filter.clone());
		kit::menu_row(
			label.to_owned(),
			kit::checkbox(move || *flag(&mut current.get())),
			"text_3",
		)
		.id(format!("inbox-filter-{key}"))
		.on_click(move || {
			let mut next = layer.filter();
			let value = flag(&mut next);
			*value = !*value;
			layer.set_filter(next);
		})
	};
	kit::menu_panel(column())
		.absolute()
		.top(parts::size(&layer.runtime, "button_height_lg"))
		.right(parts::size(&layer.runtime, "space_12"))
		.id("inbox-filter-menu")
		.child(choice("Include @everyone and @here", "everyone", |filter| {
			&mut filter.everyone
		}))
		.child(choice("Include Role Mentions", "roles", |filter| {
			&mut filter.roles
		}))
		.child(choice("This Server Only", "server", |filter| {
			&mut filter.this_server
		}))
}
