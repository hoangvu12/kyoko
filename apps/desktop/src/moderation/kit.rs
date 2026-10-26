use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::Signal;

use crate::kit::{self, Variant, menu_icon, menu_row};
pub use crate::kit::{MENU_BORDER, MENU_MARGIN, field, menu_panel, separator, switch};
use crate::timeline::TypeSize;
use crate::timeline::tok;

const CHIP_MIN: f32 = 120.;

pub fn label(value: impl Into<String>, size: &str) -> View {
	text(value.into()).type_size(size)
}

pub fn hint(value: impl Into<String>) -> View {
	kit::note(value)
}

pub fn input(name: &str, value: Signal<String>, placeholder: &str) -> View {
	kit::input(name, value, placeholder)
}

pub fn button(caption_text: &str, fill: Option<&'static str>, action: impl FnMut() + 'static) -> View {
	let variant = match fill {
		None => Variant::Link,
		Some("danger") => Variant::Critical,
		Some(_) => Variant::Primary,
	};
	kit::action(caption_text, variant, action)
}

pub fn footer(cancel: View, confirm: View) -> View {
	kit::footer(vec![cancel, confirm])
}

pub fn chips(labels: &[&'static str], selected: &Signal<usize>) -> View {
	row().w_full().flex_wrap().gap(tok("space_8")).children(
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
		.grow()
		.min_w(CHIP_MIN)
		.h(tok("button_height_sm"))
		.px(tok("space_12"))
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.border(tok("border_thickness"))
		.cursor(Cursor::Pointer)
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_sm"))
		.font_weight(500)
		.reactive_style(move || {
			if chosen.get() == index {
				Styles::new()
					.border_color(tok("button_border"))
					.bg(tok("accent_3"))
					.text_color(tok("text_0"))
			} else {
				Styles::new()
					.border_color(tok("border_light"))
					.bg(tok("bg_2"))
					.text_color(tok("text_3"))
			}
		})
		.on_click(move || {
			pick.set(index);
		})
		.child(text(caption_text.to_owned()))
}

pub fn select_anchor(shown: View, toggle: Signal<bool>) -> View {
	zgx::compose::button()
		.w_full()
		.h(tok("select_height"))
		.px(tok("space_12"))
		.gap(tok("space_8"))
		.items_center()
		.justify_between()
		.rounded(tok("radius_sm"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.bg(tok("bg_3"))
		.text_color(tok("text_3"))
		.hover(|style| style.border_color(tok("border")))
		.cursor(Cursor::Pointer)
		.on_click(move || {
			toggle.set(!toggle.with_untracked(|open| *open));
		})
		.child(
			shown
				.grow()
				.min_w(0.)
				.text_size(tok("font_size_md"))
				.line_height(tok("line_height_label")),
		)
		.child(menu_icon("chevron_down", false).text_color(tok("text_3")))
}

pub fn choice(name: &str) -> View {
	menu_row(name.to_owned(), div(), "text_3")
}

pub fn select(
	name: &str,
	labels: &'static [&'static str],
	selected: &Signal<usize>,
	open: Signal<bool>,
) -> View {
	let shown = selected.clone();
	let anchor = select_anchor(
		text_signal(move || labels.get(shown.get()).copied().unwrap_or_default().to_owned()),
		open.clone(),
	);
	let items: Vec<View> = labels
		.iter()
		.enumerate()
		.map(|(index, caption_text)| {
			let (pick, chosen) = (selected.clone(), selected.clone());
			choice(caption_text)
				.menu_checked_when(move || chosen.get() == index)
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
