use zgx::compose::prelude::*;
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::theme::Themed;

use crate::kit::{self, Variant};
use crate::timeline::tok;

pub const BUTTON_HEIGHT: f32 = 38.;
const DISABLED: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
	Primary,
	Danger,
	Ghost,
}

impl Tone {
	fn variant(self) -> Variant {
		match self {
			Tone::Primary => Variant::Primary,
			Tone::Danger => Variant::Critical,
			Tone::Ghost => Variant::Link,
		}
	}
}

pub fn clear() -> Themed<Color> {
	kit::clear()
}

pub fn panel(width: f32, title: impl Into<String>, close: impl FnMut() + 'static, body: View) -> View {
	panel_with(width, title, None, close, body)
}

pub fn panel_with(
	width: f32,
	title: impl Into<String>,
	subtitle: Option<String>,
	close: impl FnMut() + 'static,
	body: View,
) -> View {
	column()
		.w(width)
		.overflow_hidden()
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_4"))
		.text_color(tok("text_3"))
		.child(kit::header(kit::title(title), subtitle, close))
		.child(body)
}

pub fn body() -> View {
	kit::body()
}

pub fn caption(value: &str) -> View {
	kit::caption(value)
}

pub fn hint(value: impl Into<String>) -> View {
	kit::note(value)
}

pub fn field(label: &str, content: View) -> View {
	kit::field(label, content)
}

pub fn input(label: &str, value: Signal<String>, placeholder: &str) -> View {
	kit::input(label, value, placeholder)
}

pub fn on_enter(view: View, submit: impl FnMut() + 'static) -> View {
	kit::on_enter(view, submit)
}

pub fn action_button(label: impl Into<String>, tone: Tone, action: impl FnMut() + 'static) -> View {
	kit::action(label, tone.variant(), action)
}

pub fn labelled_button(label: View, tone: Tone, action: impl FnMut() + 'static) -> View {
	kit::button_view(label, tone.variant(), action)
}

pub fn footer(cancel: &str, confirm: View, on_cancel: impl FnMut() + 'static) -> View {
	kit::footer(vec![action_button(cancel, Tone::Ghost, on_cancel), confirm])
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

pub fn switch_row(title: &str, note: Option<&str>, value: Signal<bool>) -> View {
	kit::switch_row(title, note, value)
}
