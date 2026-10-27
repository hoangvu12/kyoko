use std::rc::Rc;

use bridge::ProfileCard;
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::Signal;
use zgx::style::ObjectFit;

use super::parts::{self, Badge, words};
use super::{ProfileLayer, Request, Shown, popout};
use crate::kit::{self, Variant};
use crate::timeline::icons::glyph;
use crate::timeline::pictures::Pictures;
use crate::timeline::tok;

const LIST_ICON: f32 = 40.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullTab {
	Activity,
	Friends,
	Servers,
}

impl FullTab {
	fn title(self) -> &'static str {
		match self {
			Self::Activity => "Activity",
			Self::Friends => "Mutual Friends",
			Self::Servers => "Mutual Servers",
		}
	}

	fn key(self) -> &'static str {
		match self {
			Self::Activity => "activity",
			Self::Friends => "friends",
			Self::Servers => "servers",
		}
	}

	pub fn shown(card: &ProfileCard) -> Vec<FullTab> {
		let mut tabs = Vec::new();
		if !card.activities.is_empty() {
			tabs.push(Self::Activity);
		}
		if card.mutual_friend_count > 0 {
			tabs.push(Self::Friends);
		}
		if card.mutual_server_count() > 0 {
			tabs.push(Self::Servers);
		}
		tabs
	}

	fn count(self, card: &ProfileCard) -> Option<usize> {
		match self {
			Self::Activity => None,
			Self::Friends => Some(card.mutual_friend_count),
			Self::Servers => Some(card.mutual_server_count()),
		}
	}
}

pub fn scrim(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let close = layer.clone();
	let window = layer.window.clone();
	div()
		.absolute()
		.left(0.)
		.top(0.)
		.reactive_style(move || {
			let (width, height) = window.get();
			Styles::new().w(width).h(height)
		})
		.items_center()
		.justify_center()
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
	let window = layer.window.clone();
	let gap = parts::size(&layer.runtime, "gap");
	let badge = Badge::of(
		&layer.runtime,
		card.status,
		"_xl",
		parts::size(&layer.runtime, "space_4"),
	);
	column()
		.id("full-profile")
		.relative()
		.w(tok("profile_dialog_width"))
		.h(tok("profile_dialog_height"))
		.reactive_style(move || {
			let (width, height) = window.get();
			Styles::new()
				.max_w((width - 2. * gap).max(0.))
				.max_h((height - 2. * gap).max(0.))
		})
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_3"))
		.overflow_hidden()
		.text_color(tok("text_3"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.child(
			parts::banner(
				&pictures,
				card.accent,
				images.banner.as_ref(),
				"profile_dialog_banner_height",
			)
			.child(
				div()
					.absolute()
					.top(parts::size(&layer.runtime, "space_16"))
					.right(parts::size(&layer.runtime, "space_16"))
					.child(
						parts::round_button("close", "Close", move || close.close_profile())
							.id("full-profile-close"),
					),
			),
		)
		.child(
			row()
				.w_full()
				.grow()
				.min_h(0.)
				.child(identity(layer, shown))
				.child(board(layer, shown)),
		)
		.child(
			div()
				.absolute()
				.left(parts::size(&layer.runtime, "space_20"))
				.top(parts::size(&layer.runtime, "profile_dialog_avatar_top"))
				.child(parts::avatar_ring(
					"profile_dialog_avatar_ring",
					"space_8",
					parts::avatar(
						&pictures,
						images.avatar.as_ref(),
						parts::size(&layer.runtime, "avatar_size_xl"),
						Some(badge),
						"bg_3",
					),
				)),
		)
}

fn identity(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let card = shown.card.clone();
	let images = shown.images.clone();
	let pictures = layer.pictures.clone();
	let names = column()
		.w_full()
		.child(parts::name_line(&card, "xl"))
		.child(parts::handle_line(&pictures, &card, images.tag.as_ref(), "md"))
		.when_some(
			parts::badges(&pictures, &card, &images.badges),
			|names, badges| names.child(div().mt(tok("space_8")).child(badges)),
		)
		.when_some(
			parts::status_line(&pictures, &card.custom_status, images.emoji.as_ref()),
			|names, line| names.child(div().mt(tok("space_8")).child(line)),
		);
	let mut sections: Vec<View> = vec![names, actions(layer, &card)];
	sections.extend(parts::about(&card, "Bio"));
	sections.push(parts::member_since(&card));
	sections.extend(parts::roles(&card));
	sections.extend(parts::connections(&card));
	scroll(layer.runtime.signal(0.))
		.w(tok("profile_column_width"))
		.h_full()
		.shrink_0()
		.border_edges(zgx::scene::Insets {
			right: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.child(
			column()
				.w_full()
				.pt(tok("profile_dialog_body_top"))
				.px(tok("space_24"))
				.pb(tok("space_24"))
				.gap(tok("space_16"))
				.children(sections),
		)
}

fn actions(layer: &Rc<ProfileLayer>, card: &Rc<ProfileCard>) -> View {
	let id = card.id;
	let message = layer.clone();
	row()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.when(!card.mine, |line| {
			line.child(
				kit::button_view(
					row()
						.items_center()
						.gap(tok("space_8"))
						.child(glyph("chat", 20.))
						.child(words("Message", "md")),
					Variant::Primary,
					move || {
						message.close_profile();
						message.request(Request::Message(id));
					},
				)
				.id("full-profile-message"),
			)
		})
		.child(div().grow())
		.when_some(
			popout::friend_button(layer, card, parts::icon_button::<Box<dyn FnMut()>>),
			|line, button| line.child(button),
		)
		.child(popout::more_menu(layer, card, |toggle| {
			parts::icon_button("more", "More", toggle)
		}))
}

fn board(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let tabs = FullTab::shown(&shown.card);
	let chosen = layer.tab.clone();
	let first = tabs.first().copied();
	let available = tabs.clone();
	let current = move || {
		let picked = chosen.get();
		if available.contains(&picked) {
			Some(picked)
		} else {
			first
		}
	};
	let current = Rc::new(current);
	let bar = row()
		.w_full()
		.shrink_0()
		.gap(tok("space_24"))
		.px(tok("space_24"))
		.border_edges(zgx::scene::Insets {
			bottom: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.children(
			tabs.iter()
				.map(|tab| tab_button(&layer.tab, *tab, tab.count(&shown.card), current.clone()))
				.collect::<Vec<_>>(),
		);
	let body = {
		let (layer, shown, current) = (layer.clone(), shown.clone(), current.clone());
		div().w_full().child_when(
			move || Some(current()),
			move |tab: Option<FullTab>, _| tab_body(&layer, &shown, tab),
		)
	};
	column()
		.grow()
		.min_w(0.)
		.h_full()
		.when(!tabs.is_empty(), |board| board.child(bar))
		.child(
			scroll(layer.runtime.signal(0.))
				.w_full()
				.grow()
				.min_h(0.)
				.scrollbar(true)
				.child(div().w_full().p(tok("space_24")).child(body)),
		)
}

fn tab_button(
	selected: &Signal<FullTab>,
	tab: FullTab,
	count: Option<usize>,
	current: Rc<impl Fn() -> Option<FullTab> + 'static>,
) -> View {
	let caption = match count {
		Some(count) => format!("{} \u{2014} {count}", tab.title()),
		None => tab.title().to_owned(),
	};
	let pick = selected.clone();
	div()
		.id(format!("full-profile-tab-{}", tab.key()))
		.pt(tok("space_16"))
		.pb(tok("space_14"))
		.cursor(Cursor::Pointer)
		.border_edges(zgx::scene::Insets {
			bottom: 2.,
			..Default::default()
		})
		.reactive_style(move || {
			let active = current() == Some(tab);
			Styles::new()
				.border_color(tok(if active { "accent_2" } else { "bg_3" }))
				.text_color(tok(if active { "text_2" } else { "text_4" }))
		})
		.hover(|style| style.text_color(tok("text_3")))
		.on_click(move || {
			pick.set(tab);
		})
		.child(words(caption, "md").font_weight(500))
}

fn tab_body(layer: &Rc<ProfileLayer>, shown: &Shown, tab: Option<FullTab>) -> View {
	let card = &shown.card;
	match tab {
		None => empty(if card.loading {
			"Loading profile..."
		} else {
			"Nothing to show here yet."
		}),
		Some(FullTab::Activity) => row().w_full().flex_wrap().gap(tok("space_16")).children(
			card.activities
				.iter()
				.map(|line| widget(parts::activity(line)))
				.collect::<Vec<_>>(),
		),
		Some(FullTab::Servers) => column().w_full().gap(tok("space_4")).children(
			card.mutual_guilds
				.iter()
				.zip(&shown.images.guilds)
				.map(|(guild, icon)| {
					let initial = guild.name.chars().next().map(String::from).unwrap_or_default();
					list_row(&layer.pictures, icon.as_ref(), Some(initial), &guild.name)
				})
				.collect::<Vec<_>>(),
		),
		Some(FullTab::Friends) => column().w_full().gap(tok("space_4")).children(
			card.mutual_friends
				.iter()
				.zip(&shown.images.friends)
				.map(|(friend, avatar)| list_row(&layer.pictures, avatar.as_ref(), None, &friend.name))
				.collect::<Vec<_>>(),
		),
	}
}

fn widget(content: View) -> View {
	column()
		.w(tok("profile_widget_width"))
		.p(tok("space_16"))
		.rounded(tok("profile_inner_radius"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.child(content)
}

fn list_row(pictures: &Rc<Pictures>, image: Option<&ImageName>, initial: Option<String>, name: &str) -> View {
	let circle = row()
		.size(LIST_ICON, LIST_ICON)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(LIST_ICON)
		.overflow_hidden()
		.bg(tok("bg_2"))
		.when(image.is_none(), |circle| {
			circle.when_some(initial, |circle, initial| {
				circle.child(words(initial, "sm").font_weight(600).text_color(tok("text_2")))
			})
		});
	row()
		.w_full()
		.items_center()
		.gap(tok("space_12"))
		.p(tok("space_8"))
		.rounded(tok("radius_sm"))
		.hover(|style| style.bg(tok("hover")))
		.child(parts::picture_box(pictures, image, circle, ObjectFit::Cover))
		.child(
			words(name.to_owned(), "md")
				.font_weight(600)
				.truncate()
				.min_w(0.)
				.text_color(tok("text_2")),
		)
}

fn empty(message: &str) -> View {
	words(message.to_owned(), "sm").text_color(tok("text_4"))
}
