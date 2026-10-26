use std::rc::Rc;

use bridge::{ProfileCard, Relation};
use session::{Command, FriendSource};
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::input::{EventPhase, InputEvent};

use super::parts::{self, Badge, words};
use super::{ProfileLayer, Request, Shown};
use crate::kit::{id_badge, menu_icon, menu_panel, menu_row};
use crate::timeline::icons::icon;
use crate::timeline::tok;

pub fn card(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let card = shown.card.clone();
	let images = shown.images.clone();
	let pictures = layer.pictures.clone();
	let from_avatar = layer.clone();
	let user_id = card.id;
	let identity = column()
		.w_full()
		.child(parts::name_line(&card, "lg"))
		.child(parts::handle_line(&pictures, &card, images.tag.as_ref(), "sm"))
		.when_some(
			parts::badges(&pictures, &card, &images.badges),
			|identity, badges| identity.child(div().mt(tok("space_8")).child(badges)),
		)
		.when_some(
			parts::status_line(&pictures, &card.custom_status, images.emoji.as_ref()),
			|identity, line| identity.child(div().mt(tok("space_8")).child(line)),
		);
	let badge = Badge::of(&layer.runtime, card.status, "_lg", 0.);
	column()
		.id("profile-popout")
		.relative()
		.w(tok("popout_width"))
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
			parts::banner(&pictures, card.accent, images.banner.as_ref(), "banner_height")
				.child(banner_buttons(layer, &card)),
		)
		.child(
			column()
				.w_full()
				.pt(tok("popout_body_top"))
				.px(tok("space_16"))
				.pb(tok("space_16"))
				.gap(tok("space_12"))
				.child(identity)
				.child(details(&card))
				.when(!card.mine, |body| body.child(message_field(layer, &card))),
		)
		.child(
			div()
				.absolute()
				.left(parts::size(&layer.runtime, "space_16"))
				.top(parts::size(&layer.runtime, "popout_avatar_top"))
				.cursor(Cursor::Pointer)
				.on_click(move || from_avatar.request(Request::FullProfile(user_id)))
				.child(parts::avatar_ring(
					"profile_avatar_ring",
					"space_6",
					parts::avatar(
						&pictures,
						images.avatar.as_ref(),
						parts::size(&layer.runtime, "avatar_size_lg"),
						Some(badge),
						"bg_3",
					),
				)),
		)
}

fn details(card: &ProfileCard) -> View {
	let mut top: Vec<View> = Vec::new();
	top.extend(parts::activities(card));
	top.extend(parts::about(card, "About Me"));
	let mut below: Vec<View> = vec![parts::member_since(card)];
	below.extend(parts::roles(card));
	below.extend(parts::stats(card));
	let split = !top.is_empty();
	column()
		.w_full()
		.gap(tok("space_12"))
		.p(tok("space_12"))
		.rounded(tok("profile_inner_radius"))
		.bg(tok("bg_4"))
		.children(top)
		.when(split, |card| card.child(parts::divider()))
		.children(below)
}

fn message_field(layer: &Rc<ProfileLayer>, card: &ProfileCard) -> View {
	let message = layer.clone();
	let id = card.id;
	row()
		.id("profile-popout-message")
		.w_full()
		.h(tok("input_height"))
		.items_center()
		.gap(tok("space_8"))
		.px(tok("space_12"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.border_color(tok("border")))
		.on_click(move || {
			message.close_profile();
			message.request(Request::Message(id));
		})
		.child(
			words(format!("Message @{}", card.name), "sm")
				.grow()
				.min_w(0.)
				.truncate()
				.text_color(tok("text_5")),
		)
		.child(icon("smile", 20., "text_4"))
}

fn banner_buttons(layer: &Rc<ProfileLayer>, card: &Rc<ProfileCard>) -> View {
	row()
		.absolute()
		.top(parts::size(&layer.runtime, "space_12"))
		.right(parts::size(&layer.runtime, "space_12"))
		.gap(tok("space_8"))
		.on_event(|event| {
			if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
				event.stop_propagation();
			}
		})
		.when_some(
			friend_button(layer, card, parts::round_button::<Box<dyn FnMut()>>),
			|line, button| line.child(button),
		)
		.child(more_menu(layer, card, |toggle| {
			parts::round_button("more", "More", toggle)
		}))
}

pub type DrawButton = fn(&'static str, &str, Box<dyn FnMut()>) -> View;

pub fn friend_button(layer: &Rc<ProfileLayer>, card: &ProfileCard, draw: DrawButton) -> Option<View> {
	if card.mine || card.bot {
		return None;
	}
	let friend = layer.clone();
	let (id, username) = (card.id, card.username.clone());
	match card.relation {
		Relation::None => Some(draw(
			"user_plus",
			"Add Friend",
			Box::new(move || {
				friend.request(Request::Command(Command::AddFriend {
					username: username.clone(),
					from: FriendSource::Profile,
				}));
			}),
		)),
		Relation::Incoming => Some(draw(
			"user_plus",
			"Accept Friend Request",
			Box::new(move || {
				friend.request(Request::Command(Command::AcceptFriend { user_id: id }));
			}),
		)),
		Relation::Outgoing => Some(
			draw("user_plus", "Friend Request Sent", Box::new(|| {}))
				.opacity(tok("disabled_opacity_control")),
		),
		Relation::Friend | Relation::Blocked => None,
	}
}

pub fn more_menu(
	layer: &Rc<ProfileLayer>,
	card: &ProfileCard,
	anchor: impl FnOnce(Box<dyn FnMut()>) -> View,
) -> View {
	let open = layer.runtime.signal(false);
	let toggle = open.clone();
	let anchor = anchor(Box::new(move || {
		toggle.set(!toggle.with_untracked(|open| *open));
	}));
	let id = card.id;
	let mut items = Vec::new();
	if !layer.has_full_profile() {
		let full = layer.clone();
		items.push(
			menu_row(
				"View Full Profile".to_owned(),
				menu_icon("profile", false),
				"text_3",
			)
			.on_click(move || full.request(Request::FullProfile(id))),
		);
	}
	if !card.mine && card.relation != Relation::Blocked {
		let block = layer.clone();
		items.push(
			menu_row("Block".to_owned(), menu_icon("user_x", true), "red_2").on_click(move || {
				block.close_profile();
				block.request(Request::Block(id));
			}),
		);
	}
	items.push(
		menu_row("Copy User ID".to_owned(), id_badge(), "text_3").on_click(move || {
			crate::toasts::copy(&id.to_string(), "User ID copied");
		}),
	);
	menu_panel(menu("Profile menu", open, anchor).placement(Placement {
		side: PlacementSide::Bottom,
		align: PlacementEdge::End,
		offset: 4.,
	}))
	.children(items)
}
