use std::rc::Rc;
use std::sync::Arc;

use bridge::{MessageEntry, Row};
use chrono::NaiveDate;
use protocol::MessageType;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::style::ObjectFit;
use zgx::svg::SvgData;
use zgx::theme::{Themed, TokenId};

use super::avatars::Avatars;

pub const AVATAR: f64 = 40.;
const GUTTER: f32 = 72.;
const RIGHT: f32 = 48.;
const LINE: f32 = 22.;
const BODY: f32 = 16.;
const HEADER_GAP: f32 = 16.;
const DAY_HEIGHT: f32 = 32.;
const UNREAD_HEIGHT: f32 = 29.;
const START_HEIGHT: f32 = 202.;
const SKELETON_HEIGHT: f32 = 240.;
const SKELETON_ROWS: usize = 4;
const AVERAGE_GLYPH: f32 = 7.5;
const HASH_ICON: &[u8] = include_bytes!("../../ui/icons/hash.svg");

pub fn tok(name: &str) -> TokenId {
	zgx::theme::token(&crate::theme::token(name))
}

fn color_or(color: Option<u32>, fallback: &str) -> Themed<Color> {
	match color {
		Some(color) => Themed::Value(rgb(color)),
		None => Themed::Token(tok(fallback)),
	}
}

pub struct RowEnv {
	pub avatars: Rc<Avatars>,
	pub channel_name: Signal<String>,
}

pub fn render(row: &Row, env: &RowEnv, hovered: &Signal<bool>) -> View {
	match row {
		Row::Loading => skeleton(),
		Row::Start => welcome(env),
		Row::Day(day) => day_separator(*day),
		Row::Unread => unread_marker(),
		Row::Message(entry) if entry.is_plain() && entry.grouped => follow_up(entry, hovered),
		Row::Message(entry) if entry.is_plain() => first_of_group(entry, env),
		Row::Message(entry) => system_row(entry),
	}
}

pub fn estimate(row: &Row, width: f32) -> f32 {
	match row {
		Row::Loading => SKELETON_HEIGHT,
		Row::Start => START_HEIGHT,
		Row::Day(_) => DAY_HEIGHT,
		Row::Unread => UNREAD_HEIGHT,
		Row::Message(entry) => {
			let lines = body_lines(&entry.content, width - GUTTER - RIGHT);
			let header = if entry.grouped && entry.is_plain() {
				0.
			} else {
				HEADER_GAP + LINE
			};
			header + lines * LINE + 4.
		}
	}
}

fn body_lines(content: &str, width: f32) -> f32 {
	if content.is_empty() {
		return 0.;
	}
	let per_line = (width.max(AVERAGE_GLYPH) / AVERAGE_GLYPH).floor().max(1.);
	content
		.lines()
		.map(|line| (line.chars().count() as f32 / per_line).ceil().max(1.))
		.sum()
}

fn message_frame(entry: &MessageEntry) -> View {
	column()
		.relative()
		.w_full()
		.pl(GUTTER)
		.pr(RIGHT)
		.py(2.)
		.hover(|style| style.bg(tok("message_hover")))
		.when(entry.pending, |row| row.opacity(0.5))
}

fn body(entry: &MessageEntry) -> Option<View> {
	if entry.content.is_empty() {
		return None;
	}
	let color = if entry.failed {
		tok("danger")
	} else {
		tok("text_normal")
	};
	let mut text = rich_text().child(text_span(entry.content.as_str()));
	if entry.edited {
		text = text.child(text_span(" (edited)").text_color(tok("text_muted")));
	}
	Some(
		View::from(text)
			.w_full()
			.text_size(BODY)
			.line_height(LINE)
			.text_color(color)
			.text_wrap(true),
	)
}

fn first_of_group(entry: &MessageEntry, env: &RowEnv) -> View {
	let time = bridge::message_time(entry.sent_at, bridge::today());
	let name_color = color_or(entry.author_color, "header_primary");
	let header = row()
		.items_center()
		.gap(8.)
		.h(LINE)
		.child(
			text(entry.author.as_str())
				.text_size(tok("font_md"))
				.font_weight(600)
				.text_color(name_color),
		)
		.when(entry.bot, |header| header.child(bot_tag()))
		.child(
			text(time)
				.text_size(tok("font_tiny"))
				.text_color(tok("text_muted")),
		);
	message_frame(entry)
		.mt(HEADER_GAP)
		.child(avatar(entry, env).absolute().left(16.).top(4.))
		.child(header)
		.when_some(body(entry), |row, body| row.child(body))
}

fn follow_up(entry: &MessageEntry, hovered: &Signal<bool>) -> View {
	let hovered = hovered.clone();
	let clock = row()
		.absolute()
		.left(0.)
		.top(2.)
		.w(GUTTER)
		.h(LINE)
		.items_center()
		.justify_center()
		.child(
			text(bridge::clock_text(entry.sent_at))
				.text_size(11.)
				.text_color(tok("text_muted")),
		)
		.reactive_style(move || {
			if hovered.get() {
				Styles::new().visible()
			} else {
				Styles::new().invisible()
			}
		});
	message_frame(entry)
		.child(clock)
		.when_some(body(entry), |row, body| row.child(body))
}

fn system_row(entry: &MessageEntry) -> View {
	let what = match entry.kind {
		MessageType::UserJoin => format!("{} joined the server.", entry.author),
		MessageType::ChannelPinnedMessage => format!("{} pinned a message to this channel.", entry.author),
		MessageType::ThreadCreated => format!("{} started a thread: {}", entry.author, entry.content),
		MessageType::ChannelNameChange => {
			format!("{} changed the channel name: {}", entry.author, entry.content)
		}
		_ => format!("{} did something.", entry.author),
	};
	row()
		.relative()
		.w_full()
		.items_center()
		.gap(8.)
		.pl(GUTTER)
		.pr(RIGHT)
		.mt(17.)
		.py(2.)
		.hover(|style| style.bg(tok("message_hover")))
		.child(
			text(what)
				.text_size(tok("font_md"))
				.line_height(LINE)
				.text_color(tok("text_muted"))
				.text_wrap(true),
		)
		.child(
			text(bridge::message_time(entry.sent_at, bridge::today()))
				.text_size(tok("font_tiny"))
				.text_color(tok("text_muted")),
		)
}

fn bot_tag() -> View {
	div().px(5.).rounded(tok("radius_sm")).bg(tok("brand")).child(
		text("BOT")
			.text_size(10.)
			.line_height(15.)
			.font_weight(700)
			.text_color(tok("text_on_brand")),
	)
}

fn avatar(entry: &MessageEntry, env: &RowEnv) -> View {
	let size = AVATAR as f32;
	let fallback = color_or(entry.author_color, "brand");
	let initial: String = entry
		.author
		.chars()
		.find(|character| character.is_alphanumeric())
		.map(|character| character.to_uppercase().collect())
		.unwrap_or_default();
	let picture = env
		.avatars
		.name_for(&entry.message.author)
		.map(|name| env.avatars.picture(&name));
	let circle = div()
		.size(size, size)
		.rounded(size / 2.)
		.overflow_hidden()
		.bg(fallback)
		.items_center()
		.justify_center();
	let Some(picture) = picture else {
		return circle.child(
			text(initial)
				.text_size(size * 0.42)
				.font_weight(600)
				.text_color(tok("text_on_brand")),
		);
	};
	let shown = picture.clone();
	circle
		.reactive_style(move || match shown.get() {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
			None => Styles::new(),
		})
		.child(
			text(initial)
				.text_size(size * 0.42)
				.font_weight(600)
				.text_color(tok("text_on_brand"))
				.reactive_style(move || {
					if picture.with(Option::is_some) {
						Styles::new().invisible()
					} else {
						Styles::new().visible()
					}
				}),
		)
}

fn rule(color: &str) -> View {
	div().grow().min_w(0.).h(1.).bg(tok(color))
}

fn day_separator(day: NaiveDate) -> View {
	row()
		.w_full()
		.items_center()
		.gap(8.)
		.px(16.)
		.mt(16.)
		.child(rule("divider"))
		.child(
			text(bridge::day_label(day))
				.text_size(tok("font_tiny"))
				.line_height(16.)
				.font_weight(600)
				.text_color(tok("text_muted")),
		)
		.child(rule("divider"))
}

fn unread_marker() -> View {
	row()
		.w_full()
		.items_center()
		.px(16.)
		.my(8.)
		.h(13.)
		.child(rule("danger"))
		.child(
			div()
				.px(4.)
				.h(13.)
				.rounded(tok("radius_sm"))
				.bg(tok("danger"))
				.child(
					text("NEW")
						.text_size(10.)
						.line_height(13.)
						.font_weight(700)
						.text_color(tok("text_on_brand")),
				),
		)
}

fn welcome(env: &RowEnv) -> View {
	let title = env.channel_name.clone();
	let body = env.channel_name.clone();
	let icon = SvgData::new(HASH_ICON)
		.map(|svg| Arc::new(svg.current_color()))
		.ok();
	column()
		.gap(8.)
		.px(16.)
		.pt(32.)
		.pb(16.)
		.child(
			div()
				.size(68., 68.)
				.rounded(34.)
				.bg(tok("interactive_muted"))
				.items_center()
				.justify_center()
				.text_color(tok("interactive_active"))
				.when_some(icon, |circle, icon| {
					circle.child(svg("hash", icon).size(42., 42.))
				}),
		)
		.child(
			text_signal(move || format!("Welcome to #{}!", title.get()))
				.text_size(32.)
				.line_height(40.)
				.font_weight(700)
				.text_color(tok("header_primary")),
		)
		.child(
			text_signal(move || format!("This is the start of the #{} channel.", body.get()))
				.text_size(tok("font_md"))
				.text_color(tok("text_muted")),
		)
}

fn skeleton() -> View {
	let line = |width: f32| div().h(12.).w(width).rounded(6.).bg(tok("interactive_muted"));
	column()
		.gap(16.)
		.p(16.)
		.opacity(0.6)
		.children((0..SKELETON_ROWS).map(move |index| {
			let index = index as f32;
			row()
				.gap(16.)
				.child(
					div()
						.size(AVATAR as f32, AVATAR as f32)
						.rounded(AVATAR as f32 / 2.)
						.bg(tok("interactive_muted")),
				)
				.child(
					column()
						.gap(8.)
						.child(line(90. + (index * 37.) % 80.))
						.child(line(240. + (index * 61.) % 220.))
						.when((index as usize).is_multiple_of(2), |lines| {
							lines.child(line(160.))
						}),
				)
		}))
}
