use std::rc::Rc;

use bridge::TagChip;
use zgx::compose::prelude::*;
use zgx::compose::{ModalPlacement, Placement, PlacementEdge, PlacementSide};
use zgx::reactive::Signal;

use super::popouts::{Fill, clear, close_button, wide_button};
use super::{CreateShown, FileChip, PostShown, ThreadsLayer};
use crate::timeline::{TypeSize, icon, tok};

const TOP: f32 = 80.;
const SMALL: f32 = 440.;
const MEDIUM: f32 = 540.;
const INPUT: f32 = 35.;
const SELECT: f32 = 32.;
const FOCUS_RING: f32 = 2.;
const BODY_HEIGHT: f32 = 95.;
const CHIP: f32 = 28.;
const FILE_CHIP: f32 = 36.;
const FILE_CHIP_MAX: f32 = 240.;
const STARTER_LINES: u32 = 3;
const MENU_ITEM: f32 = 28.;
const TITLE_SIZE: f32 = 18.;
const CONTENT_TOP: f32 = 5.;
const TITLE_LINE: f32 = 21.;
const CLOSE_TOP: f32 = 10.;
const CLOSE_RIGHT: f32 = 11.;
const CLOSE_HIT: f32 = 22.;
const CLOSE_GLYPH: f32 = 14.;

fn caption(label: &str) -> View {
	text(label.to_uppercase())
		.type_size("font_tiny")
		.font_weight(700)
		.text_color(tok("header_secondary"))
}

fn field(label: &str, content: View) -> View {
	column().w_full().gap(8.).child(caption(label)).child(content)
}

fn input(label: &str, placeholder: &str, value: Signal<String>) -> View {
	text_input(label, value)
		.placeholder(placeholder)
		.w_full()
		.h(INPUT)
		.px(12.)
		.rounded(tok("radius_sm"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok("input_background"))
		.text_color(tok("text_normal"))
		.type_size("font_md")
		.focus(|style| {
			style
				.border_color(tok("brand"))
				.outline_color(tok("focus_ring"))
				.outline_width(FOCUS_RING)
		})
}

fn dialog(
	label: &str,
	open: Signal<bool>,
	width: f32,
	content: View,
	on_close: impl FnMut() + 'static,
) -> View {
	modal(label.to_owned(), open)
		.modal_placement(ModalPlacement {
			x: PlacementEdge::Center,
			y: PlacementEdge::Start,
			insets: [TOP, 0., 0., 0.],
		})
		.relative()
		.w(width)
		.p(16.)
		.gap(8.)
		.rounded(tok("radius_md"))
		.bg(tok("background_primary"))
		.border(1.)
		.border_color(tok("divider"))
		.text_color(tok("text_normal"))
		.child(
			text(label.to_owned())
				.text_size(TITLE_SIZE)
				.line_height(TITLE_LINE)
				.font_weight(600)
				.text_color(tok("header_primary")),
		)
		.child(content)
		.child(
			button()
				.absolute()
				.top(CLOSE_TOP)
				.right(CLOSE_RIGHT)
				.size(CLOSE_HIT, CLOSE_HIT)
				.p(0.)
				.border(0.)
				.items_center()
				.justify_center()
				.rounded(tok("radius_sm"))
				.bg(clear())
				.cursor(Cursor::Pointer)
				.hover(|style| style.bg(tok("modifier_hover")))
				.child(icon("close", CLOSE_GLYPH, "interactive_normal"))
				.on_click(on_close),
		)
}

fn footer(
	confirm: &str,
	enabled: impl Fn() -> bool + 'static,
	on_cancel: impl FnMut() + 'static,
	on_confirm: impl FnMut() + 'static,
) -> View {
	row()
		.w_full()
		.justify_end()
		.items_center()
		.gap(8.)
		.child(wide_button("Cancel", Fill::Ghost).on_click(on_cancel))
		.child(
			wide_button(confirm, Fill::Primary)
				.disabled_when(move || !enabled())
				.disabled_style(|style| style.opacity(0.5))
				.on_click(on_confirm),
		)
}

pub fn create_dialog(layer: &Rc<ThreadsLayer>) -> View {
	let shown = layer.create.clone();
	let this = layer.clone();
	div().child_when(
		move || shown.get(),
		move |shown: CreateShown, _| create_panel(&this, &shown),
	)
}

fn create_panel(layer: &Rc<ThreadsLayer>, shown: &CreateShown) -> View {
	let (close, cancel, submit, enter) = (layer.clone(), layer.clone(), layer.clone(), layer.clone());
	let name = layer.name.clone();
	let starter = shown.preview.clone().map(|preview| {
		field(
			"Starter message",
			text(preview)
				.w_full()
				.px(12.)
				.py(8.)
				.rounded(tok("radius_sm"))
				.bg(tok("background_secondary"))
				.text_wrap(true)
				.line_clamp(STARTER_LINES)
				.type_size("font_sm")
				.text_color(tok("text_muted")),
		)
	});
	dialog(
		"Create Thread",
		layer.create_open.clone(),
		SMALL,
		column()
			.w_full()
			.pt(CONTENT_TOP)
			.gap(16.)
			.child(field(
				"Thread name",
				input("Thread name", "New Thread", layer.name.clone()).on_event(move |event| {
					if let zgx::input::InputEvent::KeyDown {
						key: zgx::input::Key::Enter,
						..
					} = event.event
					{
						enter.submit_create();
						event.prevent_default();
						event.stop_propagation();
					}
				}),
			))
			.children(starter)
			.child(field("Hide after inactivity", archive_select(layer)))
			.child(footer(
				"Create Thread",
				move || name.with(|name| !bridge::trimmed_name(name).is_empty()),
				move || cancel.close_dialogs(),
				move || submit.submit_create(),
			)),
		move || close.close_dialogs(),
	)
}

fn archive_select(layer: &Rc<ThreadsLayer>) -> View {
	let open = layer.runtime.signal(false);
	let toggle = open.clone();
	let chosen = layer.archive.clone();
	let anchor = button()
		.w_full()
		.h(SELECT)
		.px(12.)
		.items_center()
		.justify_between()
		.rounded(tok("radius_sm"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(clear())
		.cursor(Cursor::Pointer)
		.text_color(tok("text_normal"))
		.on_click(move || {
			toggle.update(|open| *open = !*open);
		})
		.child(switch(
			move || chosen.get(),
			|choice: usize, _| {
				text(bridge::ARCHIVE_CHOICES[choice].1)
					.type_size("font_sm")
					.text_color(tok("text_normal"))
			},
		))
		.child(icon("chevron_down", 16., "interactive_normal"));
	menu("Hide after inactivity", open, anchor)
		.placement(Placement {
			side: PlacementSide::Bottom,
			align: PlacementEdge::Start,
			offset: 4.,
		})
		.w(SMALL - 32.)
		.py(4.)
		.rounded(tok("radius_sm"))
		.bg(tok("background_floating"))
		.border(1.)
		.border_color(tok("divider"))
		.children(
			bridge::ARCHIVE_CHOICES
				.iter()
				.enumerate()
				.map(|(index, (_, label))| {
					let pick = layer.archive.clone();
					menu_item(*label)
						.h(MENU_ITEM)
						.px(12.)
						.py(0.)
						.child(
							text(*label)
								.text_size(tok("font_sm"))
								.text_color(tok("text_normal")),
						)
						.on_click(move || {
							pick.set(index);
						})
				})
				.collect::<Vec<_>>(),
		)
}

pub fn post_dialog(layer: &Rc<ThreadsLayer>) -> View {
	let shown = layer.post.clone();
	let this = layer.clone();
	div().child_when(
		move || shown.get(),
		move |shown: PostShown, _| post_panel(&this, &shown),
	)
}

fn post_panel(layer: &Rc<ThreadsLayer>, shown: &PostShown) -> View {
	let (close, cancel, submit, enabled, attach) = (
		layer.clone(),
		layer.clone(),
		layer.clone(),
		layer.clone(),
		layer.clone(),
	);
	let files = layer.files.clone();
	let chips_layer = layer.clone();
	let tags = (!shown.form.tags.is_empty()).then(|| {
		field(
			if shown.form.require_tag {
				"Tags (pick at least one)"
			} else {
				"Tags"
			},
			row().w_full().flex_wrap().gap(8.).children(
				shown
					.form
					.tags
					.iter()
					.map(|tag| tag_chip(layer, tag))
					.collect::<Vec<_>>(),
			),
		)
	});
	dialog(
		"Create Post",
		layer.post_open.clone(),
		MEDIUM,
		column()
			.w_full()
			.pt(CONTENT_TOP)
			.gap(16.)
			.child(field(
				"Title",
				input("Title", "Give your post a title", layer.title.clone()),
			))
			.child(field(
				"Message",
				div()
					.w_full()
					.px(12.)
					.py(8.)
					.rounded(tok("radius_md"))
					.bg(tok("input_background"))
					.child(
						text_area("Message", layer.body.clone())
							.placeholder("Write the first message of your post")
							.w_full()
							.h(BODY_HEIGHT)
							.p(8.)
							.border(0.)
							.bg(clear())
							.text_color(tok("text_normal"))
							.type_size("font_md"),
					),
			))
			.child(field(
				"Attachments",
				row()
					.w_full()
					.flex_wrap()
					.items_center()
					.gap(8.)
					.child(switch(
						move || files.get(),
						move |files: Rc<Vec<FileChip>>, _| {
							row().flex_wrap().gap(8.).children(
								files
									.iter()
									.enumerate()
									.map(|(index, file)| file_chip(&chips_layer, index, file))
									.collect::<Vec<_>>(),
							)
						},
					))
					.child(
						button()
							.h(24.)
							.px(8.)
							.py(0.)
							.gap(4.)
							.items_center()
							.rounded(tok("radius_sm"))
							.border(1.)
							.border_color(tok("divider"))
							.bg(clear())
							.cursor(Cursor::Pointer)
							.hover(|style| style.bg(tok("modifier_hover")))
							.child(icon("attach", 16., "text_normal"))
							.child(
								text("Upload a File")
									.text_size(tok("font_sm"))
									.font_weight(500)
									.text_color(tok("text_normal")),
							)
							.on_click(move || attach.pick_files()),
					),
			))
			.children(tags)
			.child(footer(
				"Post",
				move || enabled.can_post(),
				move || cancel.close_dialogs(),
				move || submit.submit_post(),
			)),
		move || close.close_dialogs(),
	)
}

fn tag_chip(layer: &Rc<ThreadsLayer>, tag: &TagChip) -> View {
	let id = tag.id;
	let (chosen, pick) = (layer.chosen.clone(), layer.chosen.clone());
	button()
		.h(CHIP)
		.px(12.)
		.py(0.)
		.items_center()
		.rounded(CHIP / 2.)
		.border(1.)
		.cursor(Cursor::Pointer)
		.text_size(tok("font_sm"))
		.reactive_style(move || {
			if chosen.with(|chosen| chosen.contains(&id)) {
				Styles::new()
					.border_color(tok("brand"))
					.bg(tok("brand_tint_strong"))
					.text_color(tok("interactive_active"))
			} else {
				Styles::new()
					.border_color(tok("divider"))
					.bg(tok("background_secondary"))
					.text_color(tok("interactive_normal"))
			}
		})
		.on_click(move || {
			pick.update(|chosen| {
				if !chosen.remove(&id) {
					chosen.insert(id);
				}
			});
		})
		.child(text(tag.label.clone()))
}

fn file_chip(layer: &Rc<ThreadsLayer>, index: usize, file: &FileChip) -> View {
	let remove = layer.clone();
	row()
		.max_w(FILE_CHIP_MAX)
		.h(FILE_CHIP)
		.pl(8.)
		.pr(4.)
		.gap(8.)
		.items_center()
		.rounded(tok("radius_sm"))
		.bg(tok("background_secondary"))
		.child(icon("file", 20., "interactive_normal"))
		.child(
			column()
				.min_w(0.)
				.child(
					text(file.name.clone())
						.truncate()
						.type_size("font_sm")
						.text_color(tok("text_normal")),
				)
				.child(
					text(file.size.clone())
						.type_size("font_tiny")
						.text_color(tok("text_muted")),
				),
		)
		.child(close_button(move || remove.remove_upload(index)).size(24., 24.))
}
