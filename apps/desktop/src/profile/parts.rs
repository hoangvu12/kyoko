use std::rc::Rc;

use bridge::{ActivityLine, DmStatus, ProfileCard, RoleChip, StatusLine};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::scene::Color;
use zgx::style::ObjectFit;
use zgx::theme::Themed;

use crate::emoji;
use crate::timeline::pictures::Pictures;
use crate::timeline::{TypeSize, tok};

pub const DIVIDER: f32 = 1.;
const ROLE_DOT: f32 = 12.;
const CHIP_HEIGHT: f32 = 24.;
const BADGE: f32 = 22.;
const EMOJI: f32 = 16.;

pub fn label(value: impl Into<String>, size: &str) -> View {
	text(value.into()).type_size(size)
}

pub fn section_label(value: &str) -> View {
	label(value.to_uppercase(), "font_tiny")
		.font_weight(700)
		.text_color(tok("header_secondary"))
}

pub fn divider() -> View {
	div().h(DIVIDER).shrink_0().bg(tok("divider"))
}

pub fn spaced_divider() -> View {
	div().py(4.).child(divider())
}

pub fn status_color(status: DmStatus) -> &'static str {
	match status {
		DmStatus::Online => "status_online",
		DmStatus::Idle => "status_idle",
		DmStatus::Dnd => "status_dnd",
		DmStatus::Offline => "status_offline",
	}
}

pub fn status_dot(status: DmStatus, size: f32, cutout: &str) -> View {
	let mark = match status {
		DmStatus::Online => None,
		DmStatus::Idle => Some(
			div()
				.absolute()
				.left(-size * 0.2)
				.top(-size * 0.2)
				.size(size * 0.7, size * 0.7)
				.rounded(size)
				.bg(tok(cutout)),
		),
		DmStatus::Dnd => Some(
			div()
				.size(size * 0.64, size * 0.26)
				.rounded(size * 0.13)
				.bg(tok(cutout)),
		),
		DmStatus::Offline => Some(div().size(size * 0.5, size * 0.5).rounded(size).bg(tok(cutout))),
	};
	row()
		.relative()
		.size(size, size)
		.shrink_0()
		.rounded(size)
		.overflow_hidden()
		.items_center()
		.justify_center()
		.bg(tok(status_color(status)))
		.when_some(mark, |dot, mark| dot.child(mark))
}

pub fn picture_box(pictures: &Rc<Pictures>, name: Option<&ImageName>, view: View, fit: ObjectFit) -> View {
	let Some(name) = name else {
		return view;
	};
	let picture = pictures.picture(name);
	view.reactive_style(move || match picture.get() {
		Some(image) => Styles::new().bg_image(image, fit),
		None => Styles::new(),
	})
}

pub fn avatar(
	pictures: &Rc<Pictures>,
	name: Option<&ImageName>,
	size: f32,
	status: Option<DmStatus>,
	cutout: &str,
) -> View {
	let circle = div()
		.size(size, size)
		.rounded(size / 2.)
		.overflow_hidden()
		.bg(tok("background_tertiary"));
	let badge = status.map(|status| {
		let outer = (size * 0.5).clamp(10., 24.);
		let ring = (outer * 0.19).round();
		row()
			.absolute()
			.right(-outer * 0.25)
			.bottom(-outer * 0.25)
			.size(outer, outer)
			.rounded(outer)
			.items_center()
			.justify_center()
			.bg(tok(cutout))
			.child(status_dot(status, outer - ring * 2., cutout))
	});
	div()
		.relative()
		.size(size, size)
		.shrink_0()
		.child(picture_box(pictures, name, circle, ObjectFit::Cover))
		.when_some(badge, |avatar, badge| avatar.child(badge))
}

pub fn avatar_ring(size: f32, ring: f32, child: View) -> View {
	div()
		.size(size + ring * 2., size + ring * 2.)
		.shrink_0()
		.rounded(size)
		.p(ring)
		.bg(tok("background_floating"))
		.child(child)
}

pub fn banner(pictures: &Rc<Pictures>, card: &ProfileCard, name: Option<&ImageName>, height: f32) -> View {
	let fill = match card.accent {
		Some(color) => Themed::Value(rgb(color)),
		None => Themed::Token(tok("brand")),
	};
	picture_box(
		pictures,
		name,
		div().h(height).w_full().shrink_0().overflow_hidden().bg(fill),
		ObjectFit::Cover,
	)
}

pub fn badges(pictures: &Rc<Pictures>, card: &ProfileCard, names: &[ImageName]) -> View {
	row().items_center().gap(4.).children(
		card.badges
			.iter()
			.zip(names)
			.map(|(badge, name)| {
				picture_box(pictures, Some(name), div().size(BADGE, BADGE), ObjectFit::Contain)
					.tooltip(badge.description.clone())
			})
			.collect::<Vec<_>>(),
	)
}

pub fn role_chip(role: &RoleChip) -> View {
	let dot = match role.color {
		Some(color) => Themed::Value(rgb(color)),
		None => Themed::Token(tok("interactive_normal")),
	};
	row()
		.items_center()
		.gap(6.)
		.h(CHIP_HEIGHT)
		.px(8.)
		.rounded(tok("radius_sm"))
		.bg(tok("background_tertiary"))
		.child(div().size(ROLE_DOT, ROLE_DOT).rounded(ROLE_DOT).bg(dot))
		.child(
			label(role.name.clone(), "font_tiny")
				.font_weight(500)
				.text_color(tok("text_normal")),
		)
}

pub fn status_line(pictures: &Rc<Pictures>, custom: &StatusLine, emoji: Option<&ImageName>) -> Option<View> {
	if custom.is_empty() {
		return None;
	}
	let glyph = custom.glyph().map(|glyph| emoji::glyph(glyph, EMOJI));
	let image =
		emoji.map(|name| picture_box(pictures, Some(name), div().size(EMOJI, EMOJI), ObjectFit::Contain));
	Some(
		row()
			.items_center()
			.gap(4.)
			.min_w(0.)
			.when_some(image, |line, image| line.child(image))
			.when_some(glyph, |line, glyph| line.child(glyph))
			.when_some(custom.text.clone(), |line, value| {
				line.child(
					label(value, "font_sm")
						.min_w(0.)
						.text_wrap(true)
						.text_color(tok("text_normal")),
				)
			}),
	)
}

pub fn activity(line: &ActivityLine) -> View {
	let detail = |value: &Option<String>| {
		value.clone().map(|value| {
			label(value, "font_xs")
				.truncate()
				.min_w(0.)
				.text_color(tok("text_muted"))
		})
	};
	column().gap(4.).child(section_label(&line.heading)).child(
		column()
			.min_w(0.)
			.child(
				label(line.name.clone(), "font_sm")
					.font_weight(600)
					.truncate()
					.min_w(0.)
					.text_color(tok("header_primary")),
			)
			.when_some(detail(&line.details), |block, line| block.child(line))
			.when_some(detail(&line.state), |block, line| block.child(line)),
	)
}

pub fn section(title: &str, body: View) -> View {
	column().gap(4.).child(section_label(title)).child(body)
}

pub fn member_since(card: &ProfileCard) -> View {
	let joined = card.joined.map(|joined| {
		label(format!("Joined {}", bridge::day_label(joined)), "font_sm").text_color(tok("text_muted"))
	});
	section(
		"Member Since",
		row()
			.flex_wrap()
			.gap(12.)
			.child(label(bridge::day_label(card.created), "font_sm").text_color(tok("text_normal")))
			.when_some(joined, |line, joined| line.child(joined)),
	)
}

pub fn about(card: &ProfileCard) -> Option<View> {
	card.bio.clone().map(|bio| {
		section(
			"About Me",
			label(bio, "font_sm")
				.text_wrap(true)
				.text_color(tok("text_normal")),
		)
	})
}

pub fn roles(card: &ProfileCard) -> Option<View> {
	(!card.roles.is_empty()).then(|| {
		column().gap(8.).child(section_label("Roles")).child(
			row()
				.flex_wrap()
				.gap(4.)
				.children(card.roles.iter().map(role_chip).collect::<Vec<_>>()),
		)
	})
}

pub fn activities(card: &ProfileCard) -> Option<View> {
	(!card.activities.is_empty()).then(|| {
		column()
			.gap(12.)
			.children(card.activities.iter().map(activity).collect::<Vec<_>>())
	})
}

pub fn plural(count: usize, one: &str) -> String {
	match count {
		1 => format!("1 {one}"),
		_ => format!("{count} {one}s"),
	}
}

pub fn stats(card: &ProfileCard) -> Option<View> {
	let servers = card.mutual_server_count();
	let friends = card.mutual_friend_count;
	if servers == 0 && friends == 0 {
		return None;
	}
	let stat = |count: usize, one: &str| {
		(count > 0).then(|| label(plural(count, one), "font_xs").text_color(tok("header_secondary")))
	};
	Some(
		row()
			.flex_wrap()
			.gap(12.)
			.when_some(stat(servers, "Mutual Server"), |line, stat| line.child(stat))
			.when_some(stat(friends, "Mutual Friend"), |line, stat| line.child(stat)),
	)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
	Primary,
	Secondary,
	Ghost,
	Link,
	Disabled,
}

pub fn small_button(caption: &str, look: Look, action: impl FnMut() + 'static) -> View {
	let (fill, color, hover) = match look {
		Look::Primary => (Some("brand"), "text_on_brand", Some("brand_hover")),
		Look::Secondary => (
			Some("button_secondary"),
			"text_normal",
			Some("button_secondary_hover"),
		),
		Look::Ghost => (None, "text_normal", Some("modifier_hover")),
		Look::Link => (None, "link", None),
		Look::Disabled => (Some("button_secondary"), "text_muted", None),
	};
	button()
		.h(24.)
		.px(if look == Look::Link { 0. } else { 8. })
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.border(0.)
		.text_size(tok("font_xs"))
		.font_weight(500)
		.text_color(tok(color))
		.bg(match fill {
			Some(fill) => Themed::Token(tok(fill)),
			None => Themed::Value(Color(0, 0, 0, 0)),
		})
		.when(look != Look::Disabled, |button| button.cursor(Cursor::Pointer))
		.when_some(hover, |button, hover| {
			button.hover(move |style| style.bg(tok(hover)))
		})
		.child(text(caption.to_owned()).when(look == Look::Link, |caption| caption.text_underline(true)))
		.on_click(action)
}
