use std::rc::Rc;

use bridge::{FORWARD_TARGETS, ForwardTarget};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Key};
use zgx::reactive::Signal;

use super::CommunityLayer;
use super::parts::{Tone, action_button, body, footer, input, panel};
use crate::timeline::{TypeSize, icon, tok};

const WIDTH: f32 = 440.;
const ROW_HEIGHT: f32 = 26.;
const LIST_MAX: f32 = 280.;
const ICON: f32 = 20.;
const CHECK: f32 = 16.;
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
	let actions = forward.clone();
	let close = layer.clone();
	let mut content = column().w_full().gap(12.).pt(8.);
	if !preview.is_empty() {
		content = content.child(
			text(preview)
				.w_full()
				.px(12.)
				.py(8.)
				.rounded(tok("radius_sm"))
				.bg(tok("background_secondary"))
				.text_wrap(true)
				.line_clamp(3)
				.type_size("font_sm")
				.text_color(tok("text_muted")),
		);
	}
	panel(
		WIDTH,
		"Forward To",
		move || close.close(),
		body().pt(0.).child(content.child(search).child(list)).child_when(
			{
				let picked = forward.picked.clone();
				move || {
					let count = picked.with(Vec::len);
					(count > 0).then_some(count)
				}
			},
			move |count: usize, _| {
				let (send, cancel) = (actions.clone(), actions.clone());
				let label = if count == 1 {
					"Send".to_owned()
				} else {
					format!("Send to {count}")
				};
				footer(
					"Cancel",
					action_button(label, Tone::Primary, move || send.send_picked()),
					move || cancel.layer.close(),
				)
			},
		),
	)
}

fn rows(forward: &Rc<Forward>, shown: &[usize]) -> View {
	if shown.is_empty() {
		return column().w_full().child(
			text(EMPTY.to_owned())
				.p(8.)
				.type_size("font_sm")
				.text_color(tok("text_muted")),
		);
	}
	let rows = shown.len() as f32;
	scroll(forward.layer.runtime.signal(0.))
		.w_full()
		.h((rows * (ROW_HEIGHT + 2.) - 2.).min(LIST_MAX))
		.child(
			column().w_full().gap(2.).children(
				shown
					.iter()
					.map(|index| row_view(forward, &forward.targets[*index])),
			),
		)
}

fn row_view(forward: &Rc<Forward>, target: &ForwardTarget) -> View {
	let channel_id = target.channel_id;
	let (toggle, chosen, marked) = (forward.clone(), forward.picked.clone(), forward.picked.clone());
	let context = target.context.clone();
	row()
		.w_full()
		.h(ROW_HEIGHT)
		.shrink_0()
		.items_center()
		.gap(8.)
		.px(8.)
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if chosen.with(|picked| picked.contains(&channel_id)) {
				Styles::new()
					.bg(tok("modifier_selected"))
					.text_color(tok("interactive_active"))
			} else {
				Styles::new().text_color(tok("interactive_normal"))
			}
		})
		.hover(|style| {
			style
				.bg(tok("modifier_hover"))
				.text_color(tok("interactive_active"))
		})
		.on_click(move || toggle.toggle(channel_id))
		.child(icon(target.icon, ICON, "text_muted"))
		.child(
			text(target.name.clone())
				.grow()
				.min_w(0.)
				.truncate()
				.type_size("font_md")
				.font_weight(600),
		)
		.child(switch(
			move || marked.with(|picked| picked.contains(&channel_id)),
			move |on: bool, _| match (on, context.clone()) {
				(true, _) => icon("check", CHECK, "brand"),
				(false, Some(context)) => text(context)
					.shrink_0()
					.type_size("font_sm")
					.text_color(tok("text_muted")),
				(false, None) => div(),
			},
		))
}
