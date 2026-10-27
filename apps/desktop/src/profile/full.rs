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
	const ALL: [FullTab; 3] = [Self::Activity, Self::Friends, Self::Servers];

	fn key(self) -> &'static str {
		match self {
			Self::Activity => "activity",
			Self::Friends => "friends",
			Self::Servers => "servers",
		}
	}

	pub fn caption(self, card: &ProfileCard) -> String {
		let counted = |count: usize, one: &str| match count {
			0 => format!("No {one}s"),
			1 => format!("1 {one}"),
			count => format!("{count} {one}s"),
		};
		match self {
			Self::Activity => "Activity".to_owned(),
			Self::Friends => counted(card.mutual_friend_count, "Mutual Friend"),
			Self::Servers => counted(card.mutual_server_count(), "Mutual Server"),
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
		.child(dialog(layer, shown))
}

fn dialog(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let close = layer.clone();
	let window = layer.window.clone();
	let gap = parts::size(&layer.runtime, "gap");
	row()
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
		.p(tok("space_24"))
		.gap(tok("space_24"))
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_4"))
		.overflow_hidden()
		.text_color(tok("text_3"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.child(identity(layer, shown))
		.child(board(layer, shown))
		.child(
			div()
				.absolute()
				.top(parts::size(&layer.runtime, "space_16"))
				.right(parts::size(&layer.runtime, "space_16"))
				.child(parts::round_button(
					"full-profile-close",
					"close",
					"Close",
					move || close.close_profile(),
				)),
		)
}

fn identity(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let card = shown.card.clone();
	let images = shown.images.clone();
	let pictures = layer.pictures.clone();
	let ring = parts::ring_fill(card.theme, "bg_3");
	let badge = Badge::of(&layer.runtime, card.status, "_lg", 0.);
	let names = column()
		.w_full()
		.child(parts::name_line(&card, "xl"))
		.child(parts::handle_line(&pictures, &card, images.tag.as_ref(), "sm"))
		.when_some(
			parts::badges(&pictures, &card, &images.badges),
			|names, badges| names.child(div().mt(tok("space_8")).child(badges)),
		)
		.when_some(
			parts::status_line(&pictures, &card.custom_status, images.emoji.as_ref()),
			|names, line| names.child(div().mt(tok("space_8")).child(line)),
		);
	let mut sections: Vec<View> = vec![names, actions(layer, shown)];
	sections.extend(bio(&card));
	sections.push(parts::member_since(&card));
	sections.extend(parts::roles(&card));
	sections.extend(parts::connections(&card));
	parts::surface(
		div()
			.id("full-profile-card")
			.relative()
			.w(tok("profile_column_width"))
			.h_full()
			.shrink_0()
			.rounded(tok("radius_lg"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.overflow_hidden(),
		card.theme,
		"bg_3",
	)
	.child(
		scroll(layer.runtime.signal(0.)).w_full().h_full().child(
			column()
				.relative()
				.w_full()
				.child(parts::banner(
					&pictures,
					card.accent,
					images.banner.as_ref(),
					"dm_profile_banner_height",
				))
				.when(card.effect, |view| {
					view.child(parts::effect_glow(
						&layer.runtime,
						card.theme,
						"dm_profile_banner_height",
					))
				})
				.child(
					column()
						.w_full()
						.pt(tok("dm_profile_content_top"))
						.px(tok("space_16"))
						.pb(tok("space_24"))
						.gap(tok("space_16"))
						.children(sections),
				)
				.child(
					div()
						.absolute()
						.left(parts::size(&layer.runtime, "space_16"))
						.top(parts::size(&layer.runtime, "dm_profile_avatar_top"))
						.child(parts::avatar_ring(
							"dm_profile_avatar_ring",
							"space_6",
							ring,
							parts::avatar_on(
								&pictures,
								images.avatar.as_ref(),
								parts::size(&layer.runtime, "avatar_size_lg"),
								Some(badge),
								ring,
							),
						)),
				),
		),
	)
}

fn bio(card: &ProfileCard) -> Option<View> {
	if card.loading {
		return Some(parts::placeholder(&[200., 140.]));
	}
	card.bio.clone().map(|bio| {
		words(bio, "sm")
			.id("full-profile-bio")
			.w_full()
			.text_wrap(true)
			.text_color(tok("text_3"))
	})
}

fn message_button(layer: &Rc<ProfileLayer>, card: &ProfileCard, labelled: bool) -> View {
	let (message, id) = (layer.clone(), card.id);
	let send = move || {
		message.close_profile();
		message.request(Request::Message(id));
	};
	if labelled {
		kit::button_view(
			row()
				.items_center()
				.gap(tok("space_8"))
				.child(glyph("chat", 20.))
				.child(words("Message", "md")),
			Variant::Primary,
			send,
		)
		.id("full-profile-message")
	} else {
		parts::icon_button("full-profile-message", "chat", "Message", send)
	}
}

fn actions(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let card = &shown.card;
	let friend = popout::friend_button(layer, card, |icon, caption, action| {
		kit::button_view(
			row()
				.items_center()
				.gap(tok("space_8"))
				.child(glyph(icon, 20.))
				.child(words(caption.to_owned(), "md")),
			Variant::Secondary,
			action,
		)
		.id("full-profile-friend")
	});
	let labelled = friend.is_none();
	row()
		.w_full()
		.items_center()
		.gap(tok("space_8"))
		.when_some(friend, |line, button| line.child(button))
		.when(!card.mine, |line| {
			line.child(message_button(layer, card, labelled))
		})
		.child(popout::more_menu(layer, shown, |toggle| {
			parts::icon_button("full-profile-more", "more", "More", toggle)
		}))
}

fn board(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let chosen = layer.tab.clone();
	let bar = row()
		.w_full()
		.shrink_0()
		.gap(tok("space_24"))
		.pr(tok("space_40"))
		.border_edges(zgx::scene::Insets {
			bottom: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.children(
			FullTab::ALL
				.iter()
				.map(|tab| tab_button(&chosen, *tab, tab.caption(&shown.card)))
				.collect::<Vec<_>>(),
		);
	let body = {
		let (layer, shown, chosen) = (layer.clone(), shown.clone(), chosen.clone());
		div().w_full().child_when(
			move || Some(chosen.get()),
			move |tab: FullTab, _| tab_body(&layer, &shown, tab),
		)
	};
	column().grow().min_w(0.).h_full().child(bar).child(
		scroll(layer.runtime.signal(0.))
			.w_full()
			.grow()
			.min_h(0.)
			.scrollbar(true)
			.child(div().w_full().py(tok("space_24")).child(body)),
	)
}

fn tab_button(selected: &Signal<FullTab>, tab: FullTab, caption: String) -> View {
	let (pick, current) = (selected.clone(), selected.clone());
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
			let active = current.get() == tab;
			Styles::new()
				.border_color(tok(if active { "accent_2" } else { "bg_4" }))
				.text_color(tok(if active { "text_2" } else { "text_4" }))
		})
		.hover(|style| style.text_color(tok("text_3")))
		.on_click(move || {
			pick.set(tab);
		})
		.child(words(caption, "md").font_weight(600))
}

fn tab_body(layer: &Rc<ProfileLayer>, shown: &Shown, tab: FullTab) -> View {
	let card = &shown.card;
	match tab {
		FullTab::Activity if card.activities.is_empty() => no_activity(layer, card),
		FullTab::Activity => row().w_full().flex_wrap().gap(tok("space_16")).children(
			card.activities
				.iter()
				.map(|line| widget(parts::activity(line)))
				.collect::<Vec<_>>(),
		),
		FullTab::Servers if card.mutual_guilds.is_empty() => empty(card, "No mutual servers"),
		FullTab::Servers => column().w_full().gap(tok("space_4")).children(
			card.mutual_guilds
				.iter()
				.zip(&shown.images.guilds)
				.map(|(guild, icon)| {
					let initial = guild.name.chars().next().map(String::from).unwrap_or_default();
					list_row(&layer.pictures, icon.as_ref(), Some(initial), &guild.name)
				})
				.collect::<Vec<_>>(),
		),
		FullTab::Friends if card.mutual_friends.is_empty() => empty(card, "No mutual friends"),
		FullTab::Friends => column().w_full().gap(tok("space_4")).children(
			card.mutual_friends
				.iter()
				.zip(&shown.images.friends)
				.map(|(friend, avatar)| list_row(&layer.pictures, avatar.as_ref(), None, &friend.name))
				.collect::<Vec<_>>(),
		),
	}
}

fn no_activity(layer: &Rc<ProfileLayer>, card: &ProfileCard) -> View {
	if card.loading {
		return empty(card, "");
	}
	let title = if card.mine {
		"You don't have any activity to share here".to_owned()
	} else {
		format!("{} doesn't have any activity to share here", card.name)
	};
	column()
		.id("full-profile-no-activity")
		.w_full()
		.pt(tok("space_56"))
		.items_center()
		.gap(tok("space_8"))
		.child(
			words(title, "md")
				.font_weight(600)
				.text_wrap(true)
				.text_color(tok("text_2")),
		)
		.child(
			words("Nothing is playing, streaming or listening right now.", "sm")
				.text_wrap(true)
				.text_color(tok("text_4")),
		)
		.when(!card.mine, |empty| {
			empty.child(div().mt(tok("space_8")).child(message_button(layer, card, true)))
		})
}

fn widget(content: View) -> View {
	column()
		.w(tok("profile_widget_width"))
		.p(tok("space_16"))
		.rounded(tok("profile_inner_radius"))
		.bg(tok("bg_3"))
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

fn empty(card: &ProfileCard, message: &str) -> View {
	let message = if card.loading {
		"Loading profile..."
	} else {
		message
	};
	words(message.to_owned(), "sm")
		.id("full-profile-empty")
		.text_color(tok("text_4"))
}
