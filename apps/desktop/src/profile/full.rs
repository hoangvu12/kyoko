use std::rc::Rc;

use media::ImageName;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::style::ObjectFit;
use zgx::theme::Themed;

use super::parts::{self, label};
use super::{ProfileLayer, Shown, popout};
use crate::timeline::icons::icon;
use crate::timeline::pictures::Pictures;
use crate::timeline::tok;

const WIDTH: f32 = 600.;
const BANNER: f32 = 120.;
const AVATAR: f32 = 120.;
const RING: f32 = 8.;
const TITLE_BAR: f32 = 22.;
const BODY_HEIGHT: f32 = 280.;
const CLOSE: f32 = 32.;
const LIST_ICON: f32 = 40.;
const SCRIM_PADDING: f32 = 32.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullTab {
	Info,
	Servers,
	Friends,
}

impl FullTab {
	fn title(self) -> &'static str {
		match self {
			Self::Info => "User Info",
			Self::Servers => "Mutual Servers",
			Self::Friends => "Mutual Friends",
		}
	}
}

pub fn scrim(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let close = layer.clone();
	let window = layer.window.clone();
	div()
		.absolute()
		.left(0.)
		.top(TITLE_BAR)
		.reactive_style(move || {
			let (width, height) = window.get();
			Styles::new().w(width).h((height - TITLE_BAR).max(0.))
		})
		.items_center()
		.justify_center()
		.p(SCRIM_PADDING)
		.bg(tok("overlay"))
		.on_event(move |event| {
			if event.phase == EventPhase::Target && matches!(event.event, InputEvent::PointerDown { .. }) {
				close.close_profile();
				event.stop_propagation();
			}
		})
		.child(card(layer, shown))
}

fn card(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let card = shown.card.clone();
	let images = shown.images.clone();
	let pictures = layer.pictures.clone();
	let close = layer.clone();
	let tab = layer.tab.clone();
	let window = layer.window.clone();
	let body = {
		let (layer, shown) = (layer.clone(), shown.clone());
		div().child_when(
			move || Some(tab.get()),
			move |tab: FullTab, _| tab_body(&layer, &shown, tab),
		)
	};
	let tabs = row()
		.gap(16.)
		.border_edges(zgx::scene::Insets {
			bottom: 1.,
			..Default::default()
		})
		.border_color(tok("divider"))
		.child(tab_button(&layer.tab, FullTab::Info, None))
		.child(tab_button(
			&layer.tab,
			FullTab::Servers,
			Some(card.mutual_server_count()),
		))
		.child(tab_button(
			&layer.tab,
			FullTab::Friends,
			Some(card.mutual_friend_count),
		));
	column()
		.relative()
		.w(WIDTH)
		.reactive_style(move || {
			let (_, height) = window.get();
			Styles::new().max_h((height - TITLE_BAR - 2. * SCRIM_PADDING).max(0.))
		})
		.rounded(tok("radius_md"))
		.bg(tok("background_floating"))
		.overflow_hidden()
		.text_color(tok("text_normal"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.child(parts::banner(&pictures, &card, images.banner.as_ref(), BANNER))
		.child(
			button()
				.absolute()
				.top(8.)
				.right(8.)
				.size(CLOSE, CLOSE)
				.p(0.)
				.border(0.)
				.items_center()
				.justify_center()
				.rounded(tok("radius_sm"))
				.bg(Themed::Value(Color(0, 0, 0, 0)))
				.hover(|style| style.bg(tok("modifier_hover")))
				.cursor(Cursor::Pointer)
				.child(icon("close", 20., "text_on_brand"))
				.on_click(move || close.close_profile()),
		)
		.child(
			row()
				.items_end()
				.justify_between()
				.px(16.)
				.mt(-AVATAR / 2.)
				.child(parts::avatar_ring(
					AVATAR,
					RING,
					parts::avatar(
						&pictures,
						images.avatar.as_ref(),
						AVATAR,
						Some(card.status),
						"background_floating",
					),
				))
				.child(div().pb(8.).child(popout::actions(layer, &card))),
		)
		.child(
			column()
				.gap(16.)
				.p(16.)
				.child(
					row()
						.items_center()
						.justify_between()
						.child(
							column()
								.min_w(0.)
								.child(
									label(card.name.clone(), "font_xl")
										.font_weight(700)
										.text_color(tok("header_primary")),
								)
								.child(
									label(card.handle_line(), "font_sm").text_color(tok("header_secondary")),
								)
								.when_some(
									parts::status_line(&pictures, &card.custom_status, images.emoji.as_ref()),
									|identity, line| identity.child(div().pt(4.).child(line)),
								),
						)
						.child(parts::badges(&pictures, &card, &images.badges)),
				)
				.child(tabs)
				.child(
					scroll(layer.runtime.signal(0.))
						.h(BODY_HEIGHT)
						.scrollbar(true)
						.child(body),
				),
		)
}

fn tab_button(selected: &Signal<FullTab>, tab: FullTab, count: Option<usize>) -> View {
	let caption = match count {
		Some(count) => format!("{} \u{2014} {count}", tab.title()),
		None => tab.title().to_owned(),
	};
	let (chosen, pick) = (selected.clone(), selected.clone());
	div()
		.px(4.)
		.pb(8.)
		.cursor(Cursor::Pointer)
		.border_edges(zgx::scene::Insets {
			bottom: 2.,
			..Default::default()
		})
		.reactive_style(move || {
			let active = chosen.get() == tab;
			Styles::new()
				.border_color(if active {
					Themed::Token(tok("brand"))
				} else {
					Themed::Value(Color(0, 0, 0, 0))
				})
				.text_color(tok(if active {
					"interactive_active"
				} else {
					"interactive_normal"
				}))
		})
		.on_click(move || {
			pick.set(tab);
		})
		.child(label(caption, "font_sm").font_weight(500))
}

fn tab_body(layer: &Rc<ProfileLayer>, shown: &Shown, tab: FullTab) -> View {
	let card = &shown.card;
	match tab {
		FullTab::Info => column()
			.gap(16.)
			.when_some(parts::activities(card), |body, activities| body.child(activities))
			.when_some(parts::about(card), |body, about| body.child(about))
			.child(parts::member_since(card))
			.when_some(parts::roles(card), |body, roles| body.child(roles))
			.when(!card.connections.is_empty(), |body| {
				body.child(
					column()
						.gap(4.)
						.child(parts::section_label("Connections"))
						.children(
							card.connections
								.iter()
								.map(|(kind, name)| {
									row()
										.gap(8.)
										.child(label(kind.clone(), "font_sm").text_color(tok("text_muted")))
										.child(label(name.clone(), "font_sm"))
								})
								.collect::<Vec<_>>(),
						),
				)
			}),
		FullTab::Servers if card.mutual_guilds.is_empty() => empty("You do not share any servers."),
		FullTab::Servers => column().gap(4.).children(
			card.mutual_guilds
				.iter()
				.zip(&shown.images.guilds)
				.map(|(guild, icon)| {
					let initial = guild.name.chars().next().map(String::from).unwrap_or_default();
					list_row(&layer.pictures, icon.as_ref(), Some(initial), &guild.name)
				})
				.collect::<Vec<_>>(),
		),
		FullTab::Friends if card.mutual_friends.is_empty() => empty("You do not have any mutual friends."),
		FullTab::Friends => column().gap(4.).children(
			card.mutual_friends
				.iter()
				.zip(&shown.images.friends)
				.map(|(friend, avatar)| list_row(&layer.pictures, avatar.as_ref(), None, &friend.name))
				.collect::<Vec<_>>(),
		),
	}
}

fn list_row(pictures: &Rc<Pictures>, image: Option<&ImageName>, initial: Option<String>, name: &str) -> View {
	let circle = row()
		.size(LIST_ICON, LIST_ICON)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(LIST_ICON)
		.overflow_hidden()
		.bg(tok("background_tertiary"))
		.when(image.is_none(), |circle| {
			circle.when_some(initial, |circle, initial| {
				circle.child(label(initial, "font_sm").font_weight(600))
			})
		});
	row()
		.items_center()
		.gap(12.)
		.p(8.)
		.rounded(tok("radius_sm"))
		.hover(|style| style.bg(tok("modifier_hover")))
		.child(parts::picture_box(pictures, image, circle, ObjectFit::Cover))
		.child(
			label(name.to_owned(), "font_md")
				.font_weight(600)
				.truncate()
				.min_w(0.),
		)
}

fn empty(message: &str) -> View {
	div()
		.p(16.)
		.child(label(message.to_owned(), "font_sm").text_color(tok("text_muted")))
}
