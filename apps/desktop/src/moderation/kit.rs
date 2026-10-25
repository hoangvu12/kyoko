use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::theme::Themed;

use crate::timeline::TypeSize;
use crate::timeline::icons::icon;
use crate::timeline::tok;

pub const MENU_WIDTH: f32 = 188.;
pub const MENU_ITEM: f32 = 28.;
pub const MENU_PADDING: f32 = 6.;
pub const MENU_BORDER: f32 = 1.;
pub const MENU_MARGIN: f32 = 8.;
pub const SEPARATOR: f32 = 9.;
pub const MENU_ICON: f32 = 12.;
const INPUT: f32 = 32.;
const INPUT_INSET: f32 = 11.;
const BUTTON: f32 = 32.;
const CHIP: f32 = 32.;
const FOCUS_RING: f32 = 2.;
const CHEVRON: f32 = 16.;
const CLOSE: f32 = 16.;

pub fn label(value: impl Into<String>, size: &str) -> View {
	text(value.into()).type_size(size)
}

pub fn caption(value: &str) -> View {
	label(value.to_uppercase(), "font_tiny")
		.font_weight(700)
		.text_color(tok("header_secondary"))
}

pub fn hint(value: impl Into<String>) -> View {
	label(value, "font_sm")
		.text_wrap(true)
		.text_color(tok("text_muted"))
}

pub fn field(caption_text: &str, content: View) -> View {
	column()
		.w_full()
		.gap(8.)
		.child(caption(caption_text))
		.child(content)
}

pub fn input(name: &str, value: Signal<String>, placeholder: &str) -> View {
	text_input(name.to_owned(), value)
		.placeholder(placeholder.to_owned())
		.w_full()
		.h(INPUT)
		.shrink_0()
		.px(INPUT_INSET)
		.rounded(tok("radius_sm"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok("input_background"))
		.text_color(tok("text_normal"))
		.type_size("font_sm")
		.focus(|style| {
			style
				.border_color(tok("brand"))
				.outline_color(tok("focus_ring"))
				.outline_width(FOCUS_RING)
		})
}

pub fn button(caption_text: &str, fill: Option<&'static str>, action: impl FnMut() + 'static) -> View {
	let color = if fill.is_some() {
		"text_on_brand"
	} else {
		"text_normal"
	};
	zgx::compose::button()
		.h(BUTTON)
		.px(16.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.bg(match fill {
			Some(fill) => Themed::Token(tok(fill)),
			None => Themed::Value(Color(0, 0, 0, 0)),
		})
		.hover(move |style| match fill {
			Some("danger") => style.bg(tok("danger_hover")),
			Some(_) => style.bg(tok("brand_hover")),
			None => style.bg(tok("modifier_hover")),
		})
		.text_color(tok(color))
		.child(label(caption_text.to_owned(), "font_sm").font_weight(500))
		.on_click(action)
}

pub fn footer(cancel: View, confirm: View) -> View {
	row()
		.w_full()
		.justify_end()
		.items_center()
		.gap(8.)
		.child(cancel)
		.child(confirm)
}

pub fn chips(labels: &[&'static str], selected: &Signal<usize>) -> View {
	row().w_full().flex_wrap().gap(8.).children(
		labels
			.iter()
			.enumerate()
			.map(|(index, caption_text)| chip(caption_text, index, selected))
			.collect::<Vec<_>>(),
	)
}

fn chip(caption_text: &'static str, index: usize, selected: &Signal<usize>) -> View {
	let (chosen, pick) = (selected.clone(), selected.clone());
	zgx::compose::button()
		.h(CHIP)
		.px(12.)
		.items_center()
		.rounded(tok("radius_sm"))
		.border(1.)
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if chosen.get() == index {
				Styles::new()
					.border_color(tok("brand"))
					.bg(tok("brand_selected"))
					.text_color(tok("interactive_active"))
			} else {
				Styles::new()
					.border_color(tok("divider"))
					.bg(tok("background_secondary"))
					.text_color(tok("interactive_normal"))
			}
		})
		.on_click(move || {
			pick.set(index);
		})
		.child(label(caption_text, "font_sm").font_weight(500))
}

pub fn select(
	name: &str,
	labels: &'static [&'static str],
	selected: &Signal<usize>,
	open: Signal<bool>,
) -> View {
	let shown = selected.clone();
	let toggle = open.clone();
	let anchor = row()
		.w_full()
		.h(BUTTON)
		.px(16.)
		.gap(8.)
		.items_center()
		.justify_between()
		.rounded(tok("radius_sm"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok("input_background"))
		.text_color(tok("text_normal"))
		.cursor(Cursor::Pointer)
		.on_click(move || {
			toggle.set(!toggle.with_untracked(|open| *open));
		})
		.child(
			text_signal(move || labels.get(shown.get()).copied().unwrap_or_default().to_owned())
				.type_size("font_md")
				.grow()
				.min_w(0.),
		)
		.child(icon("chevron_down", CHEVRON, "interactive_normal"));
	let items: Vec<View> = labels
		.iter()
		.enumerate()
		.map(|(index, caption_text)| {
			let (pick, chosen) = (selected.clone(), selected.clone());
			menu_item(*caption_text)
				.h(MENU_ITEM)
				.px(12.)
				.py(0.)
				.rounded(tok("radius_sm"))
				.menu_checked_when(move || chosen.get() == index)
				.child(label(*caption_text, "font_sm").text_color(tok("text_normal")))
				.on_click(move || {
					pick.set(index);
				})
		})
		.collect();
	menu_panel(
		menu(name.to_owned(), open, anchor)
			.placement(Placement {
				side: PlacementSide::Bottom,
				align: PlacementEdge::Start,
				offset: 4.,
			})
			.match_anchor_width(),
	)
	.w_full()
	.children(items)
}

pub fn menu_panel(panel: View) -> View {
	panel
		.w(MENU_WIDTH)
		.px(MENU_PADDING)
		.py(MENU_PADDING)
		.rounded(tok("radius_sm"))
		.bg(tok("background_floating"))
		.border(MENU_BORDER)
		.border_color(tok("divider"))
}

pub fn separator() -> View {
	div().h(1.).my((SEPARATOR - 1.) / 2.).bg(tok("divider"))
}

pub fn close_button(action: impl FnMut() + 'static) -> View {
	zgx::compose::button()
		.size(CLOSE + 8., CLOSE + 8.)
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(Themed::Value(Color(0, 0, 0, 0)))
		.hover(|style| style.bg(tok("modifier_hover")))
		.cursor(Cursor::Pointer)
		.child(icon("close", CLOSE, "interactive_normal"))
		.on_click(action)
}

pub fn switch(value: Signal<bool>) -> View {
	let (checked, flip) = (value.clone(), value);
	let thumb = checked.clone();
	zgx::compose::button()
		.relative()
		.w(36.)
		.h(20.)
		.shrink_0()
		.p(0.)
		.border(0.)
		.rounded(20.)
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if checked.get() {
				Styles::new().bg(tok("brand"))
			} else {
				Styles::new().bg(tok("interactive_muted"))
			}
		})
		.on_click(move || {
			flip.set(!flip.with_untracked(|on| *on));
		})
		.child(
			div()
				.absolute()
				.top(2.)
				.w(16.)
				.h(16.)
				.rounded(16.)
				.bg(tok("text_on_brand"))
				.reactive_style(move || Styles::new().left(if thumb.get() { 18. } else { 2. })),
		)
}
