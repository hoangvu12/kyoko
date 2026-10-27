use std::rc::Rc;

use bridge::{ActivityLine, DmStatus, ProfileCard, ProfileConnection, RoleChip, StatusLine};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::reactive::Runtime;
use zgx::style::ObjectFit;
use zgx::theme::{Themed, Themes};

use crate::emoji;
use crate::kit;
use crate::timeline::icons::{icon, plugin_icon};
use crate::timeline::pictures::Pictures;
use crate::timeline::tok;

const DECORATION: f32 = 20.;

pub fn size(runtime: &Runtime, name: &str) -> f32 {
	Themes::of(runtime).get(tok(name)).number().unwrap_or(0.)
}

pub fn words(value: impl Into<String>, scale: &str) -> View {
	let line = match scale {
		"md" => "line_height_label".to_owned(),
		other => format!("line_height_{other}"),
	};
	text(value.into())
		.text_size(tok(&format!("font_size_{scale}")))
		.line_height(tok(&line))
}

pub fn heading(value: &str) -> View {
	words(value.to_owned(), "xs")
		.font_weight(700)
		.text_color(tok("text_2"))
}

pub fn section(title: &str, gap: &str, body: View) -> View {
	column().w_full().gap(tok(gap)).child(heading(title)).child(body)
}

pub fn divider() -> View {
	div()
		.w_full()
		.h(tok("border_thickness"))
		.shrink_0()
		.bg(tok("border"))
}

pub fn status_color(status: DmStatus) -> &'static str {
	match status {
		DmStatus::Online => "online",
		DmStatus::Idle => "idle",
		DmStatus::Dnd => "dnd",
		DmStatus::Offline => "offline",
	}
}

pub fn status_dot(status: DmStatus, size: f32, cutout: &str) -> View {
	let mark = match status {
		DmStatus::Online => None,
		DmStatus::Idle => Some(
			div()
				.absolute()
				.left(-size * 0.125)
				.top(-size * 0.125)
				.size(size * 0.75, size * 0.75)
				.rounded(size)
				.bg(tok(cutout)),
		),
		DmStatus::Dnd => Some(div().size(size * 0.6, size * 0.25).rounded(size).bg(tok(cutout))),
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

#[derive(Debug, Clone, Copy)]
pub struct Badge {
	pub status: DmStatus,
	pub dot: f32,
	pub cutout: f32,
	pub inset: f32,
}

impl Badge {
	pub fn of(runtime: &Runtime, status: DmStatus, scale: &str, inset: f32) -> Self {
		Self {
			status,
			dot: size(runtime, &format!("status_size{scale}")),
			cutout: size(runtime, &format!("status_cutout{scale}")),
			inset,
		}
	}
}

pub fn avatar(
	pictures: &Rc<Pictures>,
	name: Option<&ImageName>,
	side: f32,
	badge: Option<Badge>,
	cutout: &str,
) -> View {
	let circle = div()
		.size(side, side)
		.rounded(side / 2.)
		.overflow_hidden()
		.bg(tok("bg_2"));
	let badge = badge.map(|badge| {
		let outer = badge.dot + 2. * badge.cutout;
		row()
			.absolute()
			.right(badge.inset - badge.cutout)
			.bottom(badge.inset - badge.cutout)
			.size(outer, outer)
			.rounded(outer)
			.items_center()
			.justify_center()
			.bg(tok(cutout))
			.child(status_dot(badge.status, badge.dot, cutout))
	});
	div()
		.relative()
		.size(side, side)
		.shrink_0()
		.child(picture_box(pictures, name, circle, ObjectFit::Cover))
		.when_some(badge, |avatar, badge| avatar.child(badge))
}

pub fn avatar_ring(ring: &str, padding: &str, child: View) -> View {
	row()
		.w(tok(ring))
		.h(tok(ring))
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok(ring))
		.p(tok(padding))
		.bg(tok("bg_3"))
		.child(child)
}

pub fn banner(pictures: &Rc<Pictures>, accent: Option<u32>, name: Option<&ImageName>, height: &str) -> View {
	let fill = match accent {
		Some(color) => Themed::Value(rgb(color)),
		None => Themed::Token(tok("bg_2")),
	};
	picture_box(
		pictures,
		name,
		div().relative().h(tok(height)).w_full().shrink_0().bg(fill),
		ObjectFit::Cover,
	)
}

pub fn name_line(card: &ProfileCard, scale: &str) -> View {
	row()
		.items_center()
		.gap(tok("space_6"))
		.min_w(0.)
		.child(
			words(card.name.clone(), scale)
				.font_weight(700)
				.text_color(tok("accent_1"))
				.shrink_0(),
		)
		.when_some(card.name_extra.clone(), |line, extra| {
			line.child(
				words(extra, scale)
					.font_weight(500)
					.text_color(tok("text_4"))
					.min_w(0.)
					.truncate(),
			)
		})
}

pub fn handle_line(
	pictures: &Rc<Pictures>,
	card: &ProfileCard,
	tag: Option<&ImageName>,
	scale: &str,
) -> View {
	let dot = || words("\u{2022}", scale).text_color(tok("text_5"));
	row()
		.items_center()
		.flex_wrap()
		.gap(tok("space_6"))
		.min_w(0.)
		.text_color(tok("text_3"))
		.child(words(card.handle.clone(), scale))
		.when_some(card.pronouns.clone(), |line, pronouns| {
			line.child(dot()).child(words(pronouns, scale))
		})
		.when_some(card.tag.as_ref(), |line, guild_tag| {
			line.child(tag_chip(pictures, &guild_tag.text, tag))
		})
}

pub fn tag_chip(pictures: &Rc<Pictures>, value: &str, badge: Option<&ImageName>) -> View {
	let image = badge.map(|name| {
		picture_box(
			pictures,
			Some(name),
			div().w(tok("icon_size_xs")).h(tok("icon_size_xs")),
			ObjectFit::Contain,
		)
	});
	row()
		.items_center()
		.gap(tok("space_2"))
		.h(tok("tag_height"))
		.px(tok("space_4"))
		.shrink_0()
		.rounded(tok("radius_xs"))
		.bg(tok("bg_2"))
		.when_some(image, |chip, image| chip.child(image))
		.child(
			text(value.to_owned())
				.text_size(tok("font_size_xs"))
				.line_height(tok("tag_height"))
				.font_weight(600)
				.text_color(tok("text_2")),
		)
}

pub fn badges(pictures: &Rc<Pictures>, card: &ProfileCard, names: &[ImageName]) -> Option<View> {
	let decorations = card
		.decorations
		.iter()
		.filter(|badge| badge.icon.is_some())
		.map(|badge| {
			let mark = div()
				.id(format!("profile-decoration-{}-{}", badge.plugin, badge.key))
				.child(plugin_icon(badge.icon.as_ref(), DECORATION, &badge.color));
			if badge.tooltip.is_empty() {
				mark
			} else {
				mark.tooltip(badge.tooltip.clone())
			}
		});
	let discord = card.badges.iter().zip(names).map(|(badge, name)| {
		picture_box(
			pictures,
			Some(name),
			div().w(tok("icon_size_md")).h(tok("icon_size_md")),
			ObjectFit::Contain,
		)
		.tooltip(badge.description.clone())
	});
	let marks: Vec<View> = decorations.chain(discord).collect();
	(!marks.is_empty()).then(|| row().items_center().gap(tok("space_4")).children(marks))
}

pub fn role_chip(role: &RoleChip) -> View {
	let dot = match role.color {
		Some(color) => Themed::Value(rgb(color)),
		None => Themed::Token(tok("text_4")),
	};
	row()
		.items_center()
		.gap(tok("space_4"))
		.h(tok("space_24"))
		.pl(tok("space_6"))
		.pr(tok("space_8"))
		.rounded(tok("radius_xs"))
		.bg(tok("bg_2"))
		.child(
			div()
				.w(tok("space_12"))
				.h(tok("space_12"))
				.rounded(tok("space_12"))
				.bg(dot),
		)
		.child(
			words(role.name.clone(), "xs")
				.font_weight(500)
				.text_color(tok("text_3")),
		)
}

pub fn status_line(pictures: &Rc<Pictures>, custom: &StatusLine, emoji: Option<&ImageName>) -> Option<View> {
	if custom.is_empty() {
		return None;
	}
	let side = 16.;
	let glyph = custom.glyph().map(|glyph| emoji::glyph(glyph, side));
	let image =
		emoji.map(|name| picture_box(pictures, Some(name), div().size(side, side), ObjectFit::Contain));
	Some(
		row()
			.items_center()
			.gap(tok("space_4"))
			.min_w(0.)
			.when_some(image, |line, image| line.child(image))
			.when_some(glyph, |line, glyph| line.child(glyph))
			.when_some(custom.text.clone(), |line, value| {
				line.child(
					words(value, "sm")
						.min_w(0.)
						.text_wrap(true)
						.text_color(tok("text_3")),
				)
			}),
	)
}

pub fn activity(line: &ActivityLine) -> View {
	let detail = |value: &Option<String>| {
		value
			.clone()
			.map(|value| words(value, "xs").truncate().min_w(0.).text_color(tok("text_4")))
	};
	section(
		&line.heading,
		"space_4",
		column()
			.min_w(0.)
			.child(
				words(line.name.clone(), "sm")
					.font_weight(600)
					.truncate()
					.min_w(0.)
					.text_color(tok("text_2")),
			)
			.when_some(detail(&line.details), |block, line| block.child(line))
			.when_some(detail(&line.state), |block, line| block.child(line)),
	)
}

pub fn activities(card: &ProfileCard) -> Option<View> {
	(!card.activities.is_empty()).then(|| {
		column()
			.w_full()
			.gap(tok("space_12"))
			.children(card.activities.iter().map(activity).collect::<Vec<_>>())
	})
}

pub fn about(card: &ProfileCard, title: &str) -> Option<View> {
	card.bio.clone().map(|bio| {
		section(
			title,
			"space_4",
			words(bio, "sm")
				.w_full()
				.text_wrap(true)
				.text_color(tok("text_3")),
		)
	})
}

pub fn since_line(card: &ProfileCard) -> String {
	let created = bridge::day_label(card.created);
	match (card.joined, card.guild_name.as_deref()) {
		(Some(joined), Some(guild)) => format!("{created} \u{b7} {guild}: {}", bridge::day_label(joined)),
		(Some(joined), None) => format!("{created} \u{b7} {}", bridge::day_label(joined)),
		_ => created,
	}
}

pub fn member_since(card: &ProfileCard) -> View {
	section(
		"Member Since",
		"space_4",
		words(since_line(card), "sm")
			.w_full()
			.text_wrap(true)
			.text_color(tok("text_3")),
	)
}

pub fn roles(card: &ProfileCard) -> Option<View> {
	(!card.roles.is_empty()).then(|| {
		section(
			"Roles",
			"space_8",
			row()
				.w_full()
				.flex_wrap()
				.gap(tok("space_4"))
				.children(card.roles.iter().map(role_chip).collect::<Vec<_>>()),
		)
	})
}

pub fn connections(card: &ProfileCard) -> Option<View> {
	(!card.connections.is_empty()).then(|| {
		section(
			"Connections",
			"space_8",
			column()
				.w_full()
				.gap(tok("space_8"))
				.children(card.connections.iter().map(connection).collect::<Vec<_>>()),
		)
	})
}

fn connection(account: &ProfileConnection) -> View {
	let initials: String = account.kind.chars().take(2).collect::<String>().to_uppercase();
	row()
		.w_full()
		.items_center()
		.gap(tok("space_12"))
		.px(tok("space_12"))
		.py(tok("space_8"))
		.rounded(tok("profile_inner_radius"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.child(
			row()
				.w(tok("icon_size_lg"))
				.h(tok("icon_size_lg"))
				.shrink_0()
				.items_center()
				.justify_center()
				.rounded(tok("radius_xs"))
				.bg(tok("text_1"))
				.child(words(initials, "xs").font_weight(700).text_color(tok("text_0")))
				.tooltip(account.kind.clone()),
		)
		.child(
			words(account.name.clone(), "sm")
				.grow()
				.min_w(0.)
				.truncate()
				.font_weight(600)
				.text_color(tok("text_2")),
		)
		.when(account.verified, |line| {
			line.child(icon("circle_check", 16., "green_2").tooltip("Verified"))
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
		(count > 0).then(|| words(plural(count, one), "xs").text_color(tok("text_4")))
	};
	Some(
		row()
			.flex_wrap()
			.gap(tok("space_12"))
			.when_some(stat(servers, "Mutual Server"), |line, stat| line.child(stat))
			.when_some(stat(friends, "Mutual Friend"), |line, stat| line.child(stat)),
	)
}

pub fn round_button<F: FnMut() + 'static>(glyph: &'static str, caption: &str, action: F) -> View {
	button()
		.w(tok("button_size"))
		.h(tok("button_size"))
		.shrink_0()
		.p(0.)
		.border(0.)
		.items_center()
		.justify_center()
		.rounded(tok("button_size"))
		.bg(tok("overlay_button_background"))
		.text_color(tok("text_1"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("bg_1")))
		.child(crate::timeline::icons::glyph(glyph, 20.))
		.tooltip(caption.to_owned())
		.on_click(action)
}

pub fn icon_button<F: FnMut() + 'static>(glyph: &'static str, caption: &str, action: F) -> View {
	kit::button_view(
		crate::timeline::icons::glyph(glyph, 20.),
		kit::Variant::Secondary,
		action,
	)
	.px(tok("space_10"))
	.tooltip(caption.to_owned())
}
