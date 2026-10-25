use zgx::compose::prelude::*;
use zgx::input::{InputEvent, Key};
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::theme::Themed;

use crate::timeline::{TypeSize, icon, tok};

pub const PADDING: f32 = 16.;
pub const TITLE_GAP: f32 = 8.;
pub const BODY_GAP: f32 = 16.;
pub const BUTTON_HEIGHT: f32 = 32.;
pub const INPUT_HEIGHT: f32 = 34.;
const TITLE_LINE: f32 = 20.;
const CLOSE_BOX: f32 = 24.;
const CLOSE_ICON: f32 = 16.;
const CLOSE_INSET: f32 = 8.;
const BUTTON_PADDING: f32 = 12.;
const PILL_HEIGHT: f32 = 32.;
const SWITCH_WIDTH: f32 = 36.;
const SWITCH_HEIGHT: f32 = 20.;
const SWITCH_THUMB: f32 = 16.;
const SWITCH_INSET: f32 = 2.;
const FOCUS_RING: f32 = 2.;
const DISABLED: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
	Primary,
	Danger,
	Ghost,
}

pub fn clear() -> Themed<Color> {
	Themed::Value(Color(0, 0, 0, 0))
}

pub fn panel(width: f32, title: impl Into<String>, close: impl FnMut() + 'static, body: View) -> View {
	column()
		.w(width)
		.relative()
		.pt(PADDING)
		.pb(PADDING)
		.px(PADDING)
		.gap(TITLE_GAP)
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok("background_primary"))
		.text_color(tok("text_normal"))
		.child(
			text(title.into())
				.text_size(tok("font_md"))
				.line_height(TITLE_LINE)
				.font_weight(600)
				.text_color(tok("header_primary")),
		)
		.child(body)
		.child(close_button(close))
}

fn close_button(close: impl FnMut() + 'static) -> View {
	button()
		.absolute()
		.top(CLOSE_INSET)
		.right(CLOSE_INSET)
		.size(CLOSE_BOX, CLOSE_BOX)
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(clear())
		.hover(|style| style.bg(tok("modifier_hover")))
		.cursor(Cursor::Pointer)
		.child(icon("close", CLOSE_ICON, "interactive_active"))
		.on_click(close)
}

pub fn body() -> View {
	column().w_full().gap(BODY_GAP).pt(TITLE_GAP)
}

pub fn caption(value: &str) -> View {
	text(value.to_uppercase())
		.type_size("font_tiny")
		.font_weight(700)
		.text_color(tok("header_secondary"))
}

pub fn hint(value: impl Into<String>) -> View {
	text(value.into())
		.w_full()
		.text_wrap(true)
		.type_size("font_sm")
		.text_color(tok("text_muted"))
}

pub fn field(label: &str, content: View) -> View {
	column().w_full().gap(8.).child(caption(label)).child(content)
}

pub fn input(label: &str, value: Signal<String>, placeholder: &str) -> View {
	text_input(label, value)
		.placeholder(placeholder)
		.w_full()
		.h(INPUT_HEIGHT)
		.shrink_0()
		.px(12.)
		.rounded(tok("radius_sm"))
		.border(1.)
		.border_color(tok("interactive_muted"))
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

pub fn on_enter(view: View, mut submit: impl FnMut() + 'static) -> View {
	view.on_event(move |event| {
		if let InputEvent::KeyDown { key: Key::Enter, .. } = &event.event {
			submit();
			event.prevent_default();
			event.stop_propagation();
		}
	})
}

pub fn action_button(label: impl Into<String>, tone: Tone, action: impl FnMut() + 'static) -> View {
	labelled_button(text(label.into()), tone, action)
}

pub fn labelled_button(label: View, tone: Tone, action: impl FnMut() + 'static) -> View {
	let (fill, color) = match tone {
		Tone::Primary => (Themed::Token(tok("brand")), "text_on_brand"),
		Tone::Danger => (Themed::Token(tok("danger")), "text_on_brand"),
		Tone::Ghost => (clear(), "text_normal"),
	};
	button()
		.h(BUTTON_HEIGHT)
		.px(BUTTON_PADDING)
		.shrink_0()
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.bg(fill)
		.when(tone == Tone::Ghost, |button| {
			button.hover(|style| style.bg(tok("modifier_hover")))
		})
		.text_size(tok("font_sm"))
		.font_weight(500)
		.text_color(tok(color))
		.child(label)
		.on_click(action)
}

pub fn footer(cancel: &str, confirm: View, on_cancel: impl FnMut() + 'static) -> View {
	row()
		.w_full()
		.items_center()
		.justify_end()
		.gap(8.)
		.child(action_button(cancel, Tone::Ghost, on_cancel))
		.child(confirm)
}

pub fn enabled_when(view: View, mut enabled: impl FnMut() -> bool + 'static) -> View {
	view.reactive_style(move || {
		if enabled() {
			Styles::new().opacity(1.)
		} else {
			Styles::new().opacity(DISABLED)
		}
	})
}

pub fn segmented(options: &[&str], selected: Signal<usize>) -> View {
	row()
		.flex_wrap()
		.gap(8.)
		.children(options.iter().enumerate().map(|(index, label)| {
			let (shown, pick) = (selected.clone(), selected.clone());
			button()
				.focusable(false)
				.h(PILL_HEIGHT)
				.px(12.)
				.border(1.)
				.items_center()
				.rounded(tok("radius_sm"))
				.cursor(Cursor::Pointer)
				.text_size(tok("font_sm"))
				.font_weight(600)
				.reactive_style(move || {
					if shown.get() == index {
						Styles::new()
							.border_color(tok("brand"))
							.bg(tok("brand_wash"))
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
				.child(text((*label).to_owned()))
		}))
}

pub fn switch_button(value: Signal<bool>) -> View {
	let (track, thumb, flip) = (value.clone(), value.clone(), value);
	button()
		.relative()
		.size(SWITCH_WIDTH, SWITCH_HEIGHT)
		.shrink_0()
		.p(0.)
		.border(0.)
		.rounded(SWITCH_HEIGHT)
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			Styles::new().bg(tok(if track.get() { "brand" } else { "interactive_muted" }))
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
				.bg(tok("text_on_brand"))
				.reactive_style(move || {
					let left = if thumb.get() {
						SWITCH_WIDTH - SWITCH_THUMB - SWITCH_INSET
					} else {
						SWITCH_INSET
					};
					Styles::new().left(left)
				}),
		)
}

pub fn switch_row(title: &str, note: Option<&str>, value: Signal<bool>) -> View {
	row()
		.w_full()
		.items_center()
		.justify_between()
		.gap(16.)
		.child(
			column()
				.min_w(0.)
				.grow()
				.child(
					text(title.to_owned())
						.type_size("font_md")
						.font_weight(600)
						.text_color(tok("header_primary")),
				)
				.when_some(note.map(str::to_owned), |column, note| column.child(hint(note))),
		)
		.child(switch_button(value))
}
