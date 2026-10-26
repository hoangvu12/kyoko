use std::rc::Rc;

use bridge::{FORWARD_TARGETS, ForwardTarget};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Key};
use zgx::reactive::Signal;

use super::CommunityLayer;
use super::parts::{Tone, body, enabled_when, footer, input, labelled_button, panel_with};
use crate::kit;
use crate::timeline::{TypeSize, icon, tok};

const WIDTH: f32 = 440.;
const ROW_HEIGHT: f32 = 44.;
const ROW_GAP: f32 = 2.;
const LIST_MAX: f32 = 280.;
const TILE: f32 = 32.;
const ICON: f32 = 20.;
const EMPTY: &str = "No results match your search.";

struct Forward {
	layer: Rc<CommunityLayer>,
	channel_id: Snowflake,
	message_id: Snowflake,
	targets: Vec<ForwardTarget>,
	query: Signal<String>,
	picked: Signal<Vec<Snowflake>>,
}

impl Forward {
	fn matches(&self) -> Vec<usize> {
		self.query
			.with(|query| bridge::rank_forward(&self.targets, query))
	}

	fn toggle(&self, channel_id: Snowflake) {
		let mut picked = self.picked.with_untracked(Clone::clone);
		match picked.iter().position(|id| *id == channel_id) {
			Some(index) => {
				picked.remove(index);
			}
			None if picked.len() < FORWARD_TARGETS => picked.push(channel_id),
			None => return,
		}
		self.picked.set(picked);
	}

	fn send_to(&self, targets: &[Snowflake]) {
		for to_channel_id in targets {
			self.layer.send(Command::ForwardMessage {
				channel_id: self.channel_id,
				message_id: self.message_id,
				to_channel_id: *to_channel_id,
			});
		}
		self.layer.close();
	}

	fn send_picked(&self) {
		let picked = self.picked.with_untracked(Clone::clone);
		self.send_to(&picked);
	}

	fn enter(&self) {
		let picked = self.picked.with_untracked(Clone::clone);
		if !picked.is_empty() {
			self.send_to(&picked);
			return;
		}
		let first = self
			.query
			.with_untracked(|query| bridge::rank_forward(&self.targets, query))
			.first()
			.map(|index| self.targets[*index].channel_id);
		if let Some(channel_id) = first {
			self.send_to(&[channel_id]);
		}
	}
}

pub fn view(
	layer: &Rc<CommunityLayer>,
	channel_id: Snowflake,
	message_id: Snowflake,
	preview: String,
) -> View {
	let targets = layer
		.read(|state| bridge::forward_targets(state, Utc::now()))
		.unwrap_or_default();
	let forward = Rc::new(Forward {
		layer: layer.clone(),
		channel_id,
		message_id,
		targets,
		query: layer.runtime.signal(String::new()),
		picked: layer.runtime.signal(Vec::new()),
	});
	let keys = forward.clone();
	let search = input(
		"Search channels and direct messages",
		forward.query.clone(),
		"Search channels and direct messages",
	)
	.on_event(move |event| {
		if let InputEvent::KeyDown { key: Key::Enter, .. } = &event.event {
			keys.enter();
			event.prevent_default();
			event.stop_propagation();
		}
	});
	let listed = forward.clone();
	let list = switch(
		{
			let forward = forward.clone();
			move || forward.matches()
		},
		move |shown: Vec<usize>, _| rows(&listed, &shown),
	)
	.w_full();
	let (send, cancel, count, sendable) = (
		forward.clone(),
		forward.clone(),
		forward.picked.clone(),
		forward.picked.clone(),
	);
	let close = layer.clone();
	let mut content = column().w_full().gap(tok("space_16")).child(search).child(list);
	if !preview.is_empty() {
		content = content.child(
			row()
				.w_full()
				.rounded(tok("radius_xs"))
				.overflow_hidden()
				.bg(tok("bg_3"))
				.child(
					div()
						.w(tok("divider_thickness"))
						.h_full()
						.shrink_0()
						.bg(tok("border")),
				)
				.child(
					text(preview)
						.grow()
						.min_w(0.)
						.px(tok("space_12"))
						.py(tok("space_8"))
						.text_wrap(true)
						.line_clamp(3)
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.text_color(tok("text_3")),
				),
		);
	}
	let label = text_signal(move || match count.with(Vec::len) {
		0 => "Send".to_owned(),
		picked => format!("Send ({picked})"),
	});
	let confirm = labelled_button(label, Tone::Primary, move || {
		if send.picked.with_untracked(|picked| !picked.is_empty()) {
			send.send_picked();
		}
	});
	panel_with(
		WIDTH,
		"Forward To",
		Some("Select where you want to share this message.".to_owned()),
		move || close.close(),
		body().child(content).child(footer(
			"Cancel",
			enabled_when(confirm, move || sendable.with(|picked| !picked.is_empty())),
			move || cancel.layer.close(),
		)),
	)
}

fn rows(forward: &Rc<Forward>, shown: &[usize]) -> View {
	if shown.is_empty() {
		return column().w_full().child(
			text(EMPTY.to_owned())
				.p(tok("space_8"))
				.type_size("font_sm")
				.text_color(tok("text_4")),
		);
	}
	let rows = shown.len() as f32;
	scroll(forward.layer.runtime.signal(0.))
		.w_full()
		.h((rows * (ROW_HEIGHT + ROW_GAP) - ROW_GAP).min(LIST_MAX))
		.child(
			column().w_full().gap(ROW_GAP).children(
				shown
					.iter()
					.map(|index| row_view(forward, &forward.targets[*index])),
			),
		)
}

fn row_view(forward: &Rc<Forward>, target: &ForwardTarget) -> View {
	let channel_id = target.channel_id;
	let (toggle, marked) = (forward.clone(), forward.picked.clone());
	row()
		.w_full()
		.h(ROW_HEIGHT)
		.shrink_0()
		.items_center()
		.gap(tok("space_12"))
		.px(tok("space_8"))
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("hover")))
		.on_click(move || toggle.toggle(channel_id))
		.child(
			div()
				.size(TILE, TILE)
				.shrink_0()
				.items_center()
				.justify_center()
				.rounded(tok("radius_sm"))
				.bg(tok("bg_2"))
				.child(icon(target.icon, ICON, "text_4")),
		)
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					text(target.name.clone())
						.w_full()
						.truncate()
						.text_size(tok("font_size_md"))
						.line_height(tok("line_height_label"))
						.font_weight(500)
						.text_color(tok("text_2")),
				)
				.when_some(target.context.clone(), |column, context| {
					column.child(
						text(context)
							.w_full()
							.truncate()
							.text_size(tok("font_size_xs"))
							.line_height(tok("line_height_xs"))
							.text_color(tok("text_4")),
					)
				}),
		)
		.child(kit::checkbox(move || {
			marked.with(|picked| picked.contains(&channel_id))
		}))
}
