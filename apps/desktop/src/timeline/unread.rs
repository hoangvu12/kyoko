use std::rc::Rc;

use bridge::{Row, RowKey, TimelineRow};
use chrono::{DateTime, Local, Utc};
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::icons::icon;
use super::rows::tok;

const BAR_TOP: f32 = 6.;
const BAR_SIDE: f32 = 8.;
const BAR_PADDING: f32 = 8.;
const LABEL_INSET: f32 = 4.;
const MARK_ICON: f32 = 20.;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
	pub count: usize,
	pub more: bool,
	pub since: DateTime<Utc>,
}

impl Unread {
	pub fn of(rows: &[TimelineRow], detached: bool) -> Option<Self> {
		let marker = rows.iter().position(|row| row.key == RowKey::Unread)?;
		let unread: Vec<DateTime<Utc>> = rows[marker + 1..]
			.iter()
			.filter_map(|row| match &row.row {
				Row::Message(entry) if !entry.pending && !entry.mine => Some(entry.sent_at),
				_ => None,
			})
			.collect();
		let since = *unread.first()?;
		Some(Self {
			count: unread.len(),
			more: detached,
			since,
		})
	}

	pub fn label(&self) -> String {
		let plural = if self.count == 1 && !self.more {
			"message"
		} else {
			"messages"
		};
		let more = if self.more { "+" } else { "" };
		let local = self.since.with_timezone(&Local);
		let time = bridge::clock_text(self.since);
		let since = if local.date_naive() == bridge::today() {
			time
		} else {
			format!("{time} on {}", local.format("%B %-d, %Y"))
		};
		format!("{}{more} new {plural} since {since}", self.count)
	}
}

pub struct Bar {
	pub jump: Rc<dyn Fn()>,
	pub mark_read: Rc<dyn Fn()>,
}

pub fn bar(unread: &Unread, width: Signal<(f32, f32)>, actions: &Bar) -> View {
	let (jump, mark_read) = (actions.jump.clone(), actions.mark_read.clone());
	row()
		.id("new-messages-bar")
		.absolute()
		.left(BAR_SIDE)
		.top(BAR_TOP)
		.reactive_style(move || Styles::new().w((width.get().0 - BAR_SIDE * 2.).max(0.)))
		.h(tok("new_messages_bar_height"))
		.px(BAR_PADDING)
		.items_center()
		.justify_between()
		.rounded(tok("radius_sm"))
		.border_edges(zgx::scene::Insets {
			top: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.bg(tok("accent_3"))
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_sm"))
		.font_weight(500)
		.text_color(tok("text_0"))
		.child(
			text(unread.label())
				.grow()
				.min_w(0.)
				.truncate()
				.pl(LABEL_INSET)
				.cursor(Cursor::Pointer)
				.on_click(move || jump()),
		)
		.child(
			row()
				.id("new-messages-mark-read")
				.shrink_0()
				.items_center()
				.gap(8.)
				.px(BAR_PADDING)
				.cursor(Cursor::Pointer)
				.on_click(move || mark_read())
				.child(text("Mark As Read").font_weight(600))
				.child(icon("mark_read", MARK_ICON, "text_0")),
		)
}
