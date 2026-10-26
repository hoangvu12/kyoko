use std::rc::Rc;

use bridge::MessageEntry;
use protocol::Snowflake;
use zgx::compose::prelude::*;

use super::icons::icon;
use super::rows::{RowEnv, TypeSize, tok};

pub fn thread_chip(entry: &MessageEntry, env: &Rc<RowEnv>) -> Option<View> {
	let thread = entry.message.thread.as_ref()?;
	let (actions, thread_id) = (env.actions.clone(), thread.id);
	let count = thread.message_count.unwrap_or(0);
	let activity = thread
		.last_message_id
		.map_or_else(|| thread.id.timestamp(), Snowflake::timestamp);
	let label = if count == 1 {
		"1 message".to_owned()
	} else {
		format!("{count} messages")
	};
	Some(
		row()
			.items_center()
			.gap(8.)
			.mt(4.)
			.px(12.)
			.py(8.)
			.max_w(432.)
			.rounded(tok("radius_md"))
			.border(1.)
			.border_color(tok("divider"))
			.bg(tok("background_secondary"))
			.hover(|style| style.bg(tok("modifier_selected")))
			.cursor(Cursor::Pointer)
			.on_click(move || actions.open_thread(thread_id))
			.child(icon("thread", 20., "interactive_normal"))
			.child(
				text(thread.name.clone().unwrap_or_default())
					.truncate()
					.min_w(0.)
					.type_size("font_sm")
					.font_weight(600)
					.text_color(tok("header_primary")),
			)
			.child(
				text(label)
					.shrink_0()
					.type_size("font_tiny")
					.text_color(tok("text_muted")),
			)
			.child(
				text(bridge::message_time(activity, bridge::today()))
					.shrink_0()
					.type_size("font_tiny")
					.text_color(tok("text_muted")),
			),
	)
}
