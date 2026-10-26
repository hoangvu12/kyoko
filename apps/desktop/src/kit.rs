use zgx::compose::prelude::*;
use zgx::compose::{ModalPlacement, PlacementEdge};
use zgx::input::{InputEvent, Key};
use zgx::reactive::Signal;
use zgx::scene::{Color, Direction};
use zgx::theme::Themed;

use crate::timeline::{icon, tok};

pub const MENU_WIDTH: f32 = 220.;
pub const MENU_ITEM: f32 = 32.;
pub const MENU_ICON: f32 = 16.;
pub const MENU_GAP: f32 = 2.;
pub const MENU_PADDING: f32 = 6.;
pub const MENU_BORDER: f32 = 1.;
pub const MENU_MARGIN: f32 = 8.;
pub const SEPARATOR: f32 = 9.;
pub const QUICK_REACTIONS: f32 = 44.;
const ID_BADGE_TEXT: f32 = 10.;
const CLOSE_ICON: f32 = 24.;
const CHECK_ICON: f32 = 14.;
const PADDING: f32 = 16.;
const TEXTAREA_HEIGHT: f32 = 86.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
	Primary,
	Critical,
	Secondary,
	Link,
}

pub fn clear() -> Themed<Color> {
	Themed::Value(Color(0, 0, 0, 0))
}

pub fn menu_panel(panel: View) -> View {
	panel
		.w(tok("menu_width"))
		.px(tok("space_8"))
		.py(tok("space_6"))
		.gap(tok("space_2"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
}

pub fn menu_row(label: String, glyph: View, color: &str) -> View {
	let danger = matches!(color, "danger" | "red_2");
	let tone = if danger { "red_2" } else { "text_3" };
	menu_item(label.clone())
		.h(tok("menu_item_height"))
		.px(tok("space_8"))
		.py(0.)
		.rounded(tok("radius_xs"))
		.when(danger, |item| {
			item.hover(|style| style.bg(tok("background_feedback_critical")))
		})
		.child(
			row()
				.w_full()
				.direction(Direction::Ltr)
				.items_center()
				.gap(tok("space_8"))
				.child(
					text(label)
						.grow()
						.min_w(0.)
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.font_weight(500)
						.text_color(tok(tone)),
				)
				.child(glyph),
		)
}

pub fn menu_icon(name: &'static str, danger: bool) -> View {
	icon(name, MENU_ICON, if danger { "red_2" } else { "text_4" })
}

pub fn id_badge() -> View {
	div()
		.px(tok("space_2"))
		.rounded(tok("radius_xs"))
		.border(tok("border_thickness"))
		.border_color(tok("text_4"))
		.child(
			text("ID")
				.text_size(ID_BADGE_TEXT)
				.font_weight(700)
				.text_color(tok("text_4")),
		)
}

pub fn separator() -> View {
	div()
		.h(tok("border_thickness"))
		.mx(tok("space_4"))
		.my(tok("space_4"))
		.bg(tok("border"))
}

pub fn menu_height(rows: usize, separators: usize, extra: f32) -> f32 {
	let items = rows + separators;
	rows as f32 * MENU_ITEM
		+ separators as f32 * SEPARATOR
		+ items.saturating_sub(1) as f32 * MENU_GAP
		+ extra
		+ 2. * (MENU_PADDING + MENU_BORDER)
}

pub fn centered() -> ModalPlacement {
	ModalPlacement {
		x: PlacementEdge::Center,
		y: PlacementEdge::Center,
		insets: [0.; 4],
	}
}

pub fn dialog(modal: View) -> View {
	modal
		.modal_placement(centered())
		.p(0.)
		.gap(0.)
		.overflow_hidden()
		.rounded(tok("radius_lg"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.text_color(tok("text_3"))
}

pub fn header(title: View, subtitle: Option<String>, close: impl FnMut() + 'static) -> View {
	row()
		.w_full()
		.items_start()
		.justify_between()
		.gap(tok("space_16"))
		.pt(tok("space_16"))
		.px(tok("space_16"))
		.child(
			column()
				.grow()
				.min_w(0.)
				.gap(tok("space_4"))
				.child(title)
				.when_some(subtitle, |column, subtitle| {
					column.child(
						text(subtitle)
							.w_full()
							.text_wrap(true)
							.text_size(tok("font_size_sm"))
							.line_height(tok("line_height_sm"))
							.text_color(tok("text_4")),
					)
				}),
		)
		.child(close_button(close))
}

pub fn title(value: impl Into<String>) -> View {
	title_view(text(value.into()))
}

pub fn title_view(view: View) -> View {
	view.w_full()
		.text_wrap(true)
		.text_size(tok("font_size_lg"))
		.line_height(tok("line_height_lg"))
		.font_weight(600)
		.text_color(tok("text_2"))
}

pub fn close_button(action: impl FnMut() + 'static) -> View {
	button()
		.focusable(false)
		.size(CLOSE_ICON, CLOSE_ICON)
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.bg(clear())
		.cursor(Cursor::Pointer)
		.text_color(tok("text_4"))
		.hover(|style| style.text_color(tok("text_3")))
		.child(crate::timeline::icons::glyph("close", CLOSE_ICON))
		.on_click(action)
}

pub fn body() -> View {
	column().w_full().gap(tok("space_16")).p(tok("space_16"))
}

pub fn footer(buttons: Vec<View>) -> View {
	footer_band(
		row()
			.w_full()
			.items_center()
			.justify_end()
			.gap(tok("space_8"))
			.children(buttons),
	)
}

pub fn footer_band(content: View) -> View {
	column()
		.w_full()
		.mx(-PADDING)
		.mb(-PADDING)
		.child(div().w_full().h(tok("border_thickness")).bg(tok("border")))
		.child(div().w_full().p(tok("space_16")).bg(tok("bg_4")).child(content))
}

pub fn header_with(title: View, subtitle: View, close: impl FnMut() + 'static) -> View {
	row()
		.w_full()
		.items_start()
		.justify_between()
		.gap(tok("space_16"))
		.pt(tok("space_16"))
		.px(tok("space_16"))
		.child(
			column()
				.grow()
				.min_w(0.)
				.gap(tok("space_4"))
				.child(title)
				.child(subtitle),
		)
		.child(close_button(close))
}

pub fn paragraph(value: impl Into<String>) -> View {
	text(value.into())
		.w_full()
		.text_wrap(true)
		.text_size(tok("font_size_md"))
		.line_height(tok("line_height_label"))
		.text_color(tok("text_3"))
}

pub fn note(value: impl Into<String>) -> View {
	text(value.into())
		.w_full()
		.text_wrap(true)
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_sm"))
		.text_color(tok("text_4"))
}

pub fn caption_note(value: impl Into<String>) -> View {
	text(value.into())
		.w_full()
		.mt(tok("space_6"))
		.text_wrap(true)
		.text_size(tok("font_size_xs"))
		.line_height(tok("line_height_xs"))
		.text_color(tok("text_5"))
}

pub fn caption(value: &str) -> View {
	text(value.to_uppercase())
		.text_size(tok("font_size_xs"))
		.line_height(tok("line_height_xs"))
		.font_weight(700)
		.text_color(tok("text_4"))
}

pub fn field(label: &str, content: View) -> View {
	column()
		.w_full()
		.gap(tok("space_8"))
		.child(caption(label))
		.child(content)
}

pub fn button_view(label: View, variant: Variant, action: impl FnMut() + 'static) -> View {
	let base = button()
		.h(tok("button_height_md"))
		.px(tok("space_16"))
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_label"))
		.font_weight(500)
		.child(label)
		.on_click(action);
	match variant {
		Variant::Primary => filled(base, ["accent_3", "accent_4", "accent_5"]),
		Variant::Critical => filled(base, ["red_3", "red_4", "red_5"]),
		Variant::Secondary => base
			.bg(tok("bg_2"))
			.border(tok("border_thickness"))
			.border_color(tok("border_light"))
			.text_color(tok("text_3"))
			.hover(|style| style.bg(tok("bg_1")))
			.active(|style| style.bg(tok("bg_1"))),
		Variant::Link => base
			.focusable(false)
			.bg(clear())
			.border(0.)
			.text_color(tok("text_3"))
			.hover(|style| style.text_color(tok("text_2"))),
	}
}

fn filled(view: View, [rest, hovered, pressed]: [&'static str; 3]) -> View {
	view.bg(tok(rest))
		.border(tok("border_thickness"))
		.border_color(tok("button_border"))
		.text_color(tok("text_0"))
		.hover(move |style| style.bg(tok(hovered)))
		.active(move |style| style.bg(tok(pressed)))
}

pub fn action(label: impl Into<String>, variant: Variant, action: impl FnMut() + 'static) -> View {
	button_view(text(label.into()), variant, action)
}

pub fn input(label: &str, value: Signal<String>, placeholder: &str) -> View {
	text_input(label.to_owned(), value)
		.placeholder(placeholder.to_owned())
		.w_full()
		.h(tok("input_height"))
		.shrink_0()
		.px(tok("space_12"))
		.rounded(tok("radius_sm"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.bg(tok("bg_3"))
		.text_color(tok("text_3"))
		.text_size(tok("font_size_md"))
		.hover(|style| style.border_color(tok("border")))
		.focus(|style| style.border_color(tok("accent_2")).outline_width(0.))
}

pub fn textarea(label: &str, value: Signal<String>, placeholder: &str) -> View {
	text_area(label.to_owned(), value)
		.placeholder(placeholder.to_owned())
		.w_full()
		.h(TEXTAREA_HEIGHT)
		.shrink_0()
		.px(tok("space_12"))
		.py(tok("space_10"))
		.rounded(tok("radius_sm"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.bg(tok("bg_3"))
		.text_color(tok("text_3"))
		.text_size(tok("font_size_md"))
		.line_height(tok("line_height_md"))
		.hover(|style| style.border_color(tok("border")))
		.focus(|style| style.border_color(tok("accent_2")).outline_width(0.))
}

pub fn on_enter(view: View, mut submit: impl FnMut() + 'static) -> View {
	view.on_event(move |event| {
		if let InputEvent::KeyDown {
			key: Key::Enter,
			modifiers,
			..
		} = &event.event
			&& !modifiers.shift
		{
			submit();
			event.prevent_default();
			event.stop_propagation();
		}
	})
}

pub fn switch(value: Signal<bool>) -> View {
	let (track, thumb, flip) = (value.clone(), value.clone(), value);
	button()
		.relative()
		.w(tok("switch_width"))
		.h(tok("switch_height"))
		.shrink_0()
		.p(0.)
		.rounded(tok("switch_height"))
		.border(tok("border_thickness"))
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if track.get() {
				Styles::new()
					.bg(tok("accent_3"))
					.border_color(tok("button_border"))
			} else {
				Styles::new().bg(tok("bg_3")).border_color(tok("border"))
			}
		})
		.on_click(move || {
			flip.set(!flip.with_untracked(|on| *on));
		})
		.child(
			div()
				.absolute()
				.top(SWITCH_INSET)
				.size(SWITCH_THUMB, SWITCH_THUMB)
				.rounded(SWITCH_THUMB)
				.reactive_style(move || {
					if thumb.get() {
						Styles::new()
							.left(SWITCH_WIDTH - 2. * MENU_BORDER - SWITCH_THUMB - SWITCH_INSET)
							.bg(tok("bg_4"))
					} else {
						Styles::new().left(SWITCH_INSET).bg(tok("text_3"))
					}
				}),
		)
}

const SWITCH_WIDTH: f32 = 48.;
const SWITCH_HEIGHT: f32 = 24.;
const SWITCH_THUMB: f32 = 14.;
const SWITCH_INSET: f32 = (SWITCH_HEIGHT - 2. * MENU_BORDER - SWITCH_THUMB) / 2.;

pub fn switch_row(title: &str, note_text: Option<&str>, value: Signal<bool>) -> View {
	row()
		.w_full()
		.items_center()
		.gap(tok("space_16"))
		.child(
			column()
				.grow()
				.min_w(0.)
				.gap(tok("space_2"))
				.child(
					text(title.to_owned())
						.text_size(tok("font_size_md"))
						.line_height(tok("line_height_label"))
						.font_weight(600)
						.text_color(tok("text_2")),
				)
				.when_some(note_text.map(str::to_owned), |column, value| {
					column.child(note(value))
				}),
		)
		.child(switch(value))
}

pub fn radio_row(
	label: &str,
	selected: impl Fn() -> bool + Clone + 'static,
	pick: impl FnMut() + 'static,
) -> View {
	let (row_on, dot_on, text_on) = (selected.clone(), selected.clone(), selected);
	button()
		.w_full()
		.items_center()
		.justify_start()
		.gap(tok("space_12"))
		.px(tok("space_12"))
		.py(tok("space_10"))
		.border(0.)
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.reactive_style(move || Styles::new().bg(tok(if row_on() { "active" } else { "bg_3" })))
		.on_click(pick)
		.child(
			div()
				.size(RADIO, RADIO)
				.shrink_0()
				.rounded(RADIO)
				.reactive_style(move || {
					if dot_on() {
						Styles::new()
							.bg(tok("bg_4"))
							.border(tok("radio_border_selected"))
							.border_color(tok("accent_2"))
					} else {
						Styles::new()
							.bg(tok("bg_2"))
							.border(tok("border_thickness"))
							.border_color(tok("border"))
					}
				}),
		)
		.child(
			text(label.to_owned())
				.text_size(tok("font_size_md"))
				.line_height(tok("line_height_label"))
				.font_weight(500)
				.reactive_style(move || {
					Styles::new().text_color(tok(if text_on() { "text_2" } else { "text_3" }))
				}),
		)
}

const RADIO: f32 = 20.;

pub fn checkbox(checked: impl Fn() -> bool + 'static) -> View {
	let shown = std::rc::Rc::new(checked);
	let mark = shown.clone();
	div()
		.size(RADIO, RADIO)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_xs"))
		.border(tok("border_thickness"))
		.reactive_style(move || {
			if shown() {
				Styles::new()
					.bg(tok("accent_2"))
					.border_color(tok("button_border"))
			} else {
				Styles::new().bg(tok("bg_2")).border_color(tok("border"))
			}
		})
		.child(
			crate::timeline::icons::glyph("check", CHECK_ICON)
				.text_color(tok("text_0"))
				.reactive_style(move || Styles::new().opacity(if mark() { 1. } else { 0. })),
		)
}

pub fn divider() -> View {
	div().w_full().h(tok("border_thickness")).bg(tok("border"))
}
