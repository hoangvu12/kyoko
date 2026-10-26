use std::rc::Rc;

use bridge::CloneTarget;
use media::ImageName;
use protocol::StickerItem;
use state::Expression;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::parts::{Tone, action_button, body, field, hint, input, on_enter, panel};
use super::{CommunityLayer, Request};
use crate::pickers::portrait;
use crate::timeline::{TypeSize, tok};

const WIDTH: f32 = 440.;
const EMOJI_PREVIEW: f32 = 48.;
const STICKER_PREVIEW: f32 = 96.;
const ROW_HEIGHT: f32 = 44.;
const ROW_GAP: f32 = 2.;
const ICON: f32 = 32.;
const LIST_MAX: f32 = 236.;
const BLOCKED: f32 = 0.5;
const NO_SERVERS: &str = "You can't add emoji or stickers to any of your servers.";

struct Clone {
	layer: Rc<CommunityLayer>,
	expression: Expression,
	name: Signal<String>,
	problem: Signal<Option<String>>,
}

impl Clone {
	fn pick(&self, target: &CloneTarget) {
		if target.blocked.is_some() {
			return;
		}
		let name = self.name.with_untracked(|name| name.trim().to_owned());
		if let Some(problem) = bridge::clone_name_problem(&self.expression, &name) {
			self.problem.set(Some(problem));
			return;
		}
		self.layer.request(Request::Clone {
			expression: self.expression.clone(),
			guild_id: target.guild_id,
			server: target.name.clone(),
			name,
		});
		self.layer.close();
	}
}

pub fn title(expression: &Expression) -> &'static str {
	if expression.is_sticker() {
		"Clone Sticker"
	} else {
		"Clone Emoji"
	}
}

pub fn view(layer: &Rc<CommunityLayer>, expression: Expression) -> View {
	let targets = layer
		.read(|state| bridge::clone_targets(state, &expression))
		.unwrap_or_default();
	let preview = preview_name(layer, &expression);
	let mut images: Vec<ImageName> = preview.iter().cloned().collect();
	images.extend(targets.iter().filter_map(|target| {
		let hash = target.icon.as_deref()?;
		Some(layer.pictures.guild_icon(target.guild_id, hash, ICON))
	}));
	layer.set_images(images);
	let clone = Rc::new(Clone {
		layer: layer.clone(),
		name: layer.runtime.signal(bridge::clone_name(&expression)),
		problem: layer.runtime.signal(None),
		expression,
	});
	let side = if clone.expression.is_sticker() {
		STICKER_PREVIEW
	} else {
		EMOJI_PREVIEW
	};
	let first = targets.iter().find(|target| target.blocked.is_none()).cloned();
	let entered = clone.clone();
	let name_field = on_enter(input("Name", clone.name.clone(), "Name"), move || {
		if let Some(first) = &first {
			entered.pick(first);
		}
	});
	let problem = clone.problem.clone();
	let close = layer.clone();
	let cancel = layer.clone();
	panel(
		WIDTH,
		title(&clone.expression),
		move || close.close(),
		body()
			.child(
				row()
					.w_full()
					.items_center()
					.gap(16.)
					.child(preview_view(layer, preview, side))
					.child(column().grow().min_w(0.).child(field("Name", name_field))),
			)
			.child_when(
				move || problem.get(),
				|problem: String, _| {
					text(problem)
						.w_full()
						.text_wrap(true)
						.type_size("font_sm")
						.text_color(tok("danger"))
				},
			)
			.child(field("Add to", server_list(&clone, &targets)))
			.child(
				row()
					.w_full()
					.justify_end()
					.child(action_button("Cancel", Tone::Ghost, move || cancel.close())),
			),
	)
}

fn preview_name(layer: &CommunityLayer, expression: &Expression) -> Option<ImageName> {
	match expression {
		Expression::Emoji { id, animated, .. } => {
			Some(layer.pictures.emoji(id.0, *animated, f64::from(EMOJI_PREVIEW)))
		}
		Expression::Sticker { id, name, format } => layer.pictures.sticker_cell(
			&StickerItem {
				id: *id,
				name: name.clone(),
				format_type: Some(*format),
			},
			STICKER_PREVIEW,
		),
	}
}

fn preview_view(layer: &CommunityLayer, name: Option<ImageName>, side: f32) -> View {
	let picture = name.map(|name| layer.pictures.picture(&name));
	div()
		.size(side, side)
		.shrink_0()
		.reactive_style(move || match picture.as_ref().and_then(Signal::get) {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
			None => Styles::new(),
		})
}

fn server_list(clone: &Rc<Clone>, targets: &[CloneTarget]) -> View {
	if targets.is_empty() {
		return hint(NO_SERVERS);
	}
	let rows = targets.len() as f32;
	scroll(clone.layer.runtime.signal(0.))
		.w_full()
		.h((rows * (ROW_HEIGHT + ROW_GAP) - ROW_GAP).min(LIST_MAX))
		.child(
			column()
				.w_full()
				.gap(ROW_GAP)
				.children(targets.iter().map(|target| server_row(clone, target))),
		)
}

fn server_row(clone: &Rc<Clone>, target: &CloneTarget) -> View {
	let blocked = target.blocked.is_some();
	let detail = match (&target.blocked, target.slots) {
		(Some(reason), _) => reason.clone(),
		(None, Some(slots)) if slots.free() == 1 => "1 slot left".to_owned(),
		(None, Some(slots)) => format!("{} slots left", slots.free()),
		(None, None) => String::new(),
	};
	let (picker, picked) = (clone.clone(), target.clone());
	row()
		.id(format!("clone-server-{}", target.guild_id))
		.w_full()
		.h(ROW_HEIGHT)
		.shrink_0()
		.items_center()
		.gap(12.)
		.px(8.)
		.rounded(tok("radius_sm"))
		.text_color(tok("interactive_normal"))
		.when(blocked, |row| row.opacity(BLOCKED))
		.when(!blocked, |row| {
			row.cursor(Cursor::Pointer)
				.hover(|style| {
					style
						.bg(tok("modifier_hover"))
						.text_color(tok("interactive_active"))
				})
				.on_click(move || picker.pick(&picked))
		})
		.child(portrait(
			&clone.layer.pictures,
			target.guild_id,
			target.icon.as_deref(),
			&target.name,
			ICON,
		))
		.child(
			text(target.name.clone())
				.grow()
				.min_w(0.)
				.truncate()
				.type_size("font_md")
				.font_weight(600),
		)
		.child(
			text(detail)
				.shrink_0()
				.type_size("font_sm")
				.text_color(tok(if blocked { "danger" } else { "text_muted" })),
		)
}
