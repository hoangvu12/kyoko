use std::rc::Rc;

use bridge::{ProfileCard, Relation};
use session::{Command, FriendSource};
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};

use super::parts::{self, Look, label};
use super::{ProfileLayer, Request, Shown};
use crate::timeline::tok;

const WIDTH: f32 = 300.;
const BANNER: f32 = 60.;
const AVATAR: f32 = 80.;
const RING: f32 = 4.;

pub fn card(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
	let card = shown.card.clone();
	let images = shown.images.clone();
	let pictures = layer.pictures.clone();
	let full = layer.clone();
	let user_id = card.id;
	let identity = column()
		.child(
			label(card.name.clone(), "font_lg")
				.font_weight(700)
				.text_color(tok("header_primary")),
		)
		.child(label(card.handle_line(), "font_sm").text_color(tok("header_secondary")))
		.when_some(
			parts::status_line(&pictures, &card.custom_status, images.emoji.as_ref()),
			|identity, line| identity.child(div().pt(4.).child(line)),
		);
	column()
		.w(WIDTH)
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok("divider"))
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
			row()
				.items_start()
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
				.child(
					div()
						.pt(AVATAR / 2. + 8.)
						.child(parts::badges(&pictures, &card, &images.badges)),
				),
		)
		.child(
			column()
				.gap(12.)
				.p(16.)
				.child(identity)
				.child(parts::divider())
				.when_some(parts::activities(&card), |body, activities| {
					body.child(activities)
				})
				.when_some(parts::about(&card), |body, about| body.child(about))
				.child(parts::member_since(&card))
				.when_some(parts::roles(&card), |body, roles| body.child(roles))
				.when_some(parts::stats(&card), |body, stats| body.child(stats))
				.child(actions(layer, &card))
				.child(row().justify_center().child(parts::small_button(
					"View Full Profile",
					Look::Link,
					move || {
						full.request(Request::FullProfile(user_id));
					},
				))),
		)
}

pub fn actions(layer: &Rc<ProfileLayer>, card: &Rc<ProfileCard>) -> View {
	let id = card.id;
	let mut actions = row().flex_wrap().items_center().gap(8.);
	if !card.mine {
		let message = layer.clone();
		actions = actions.child(parts::small_button("Message", Look::Primary, move || {
			message.close_profile();
			message.request(Request::Message(id));
		}));
		if !card.bot {
			let friend = layer.clone();
			let username = card.username.clone();
			actions = match card.relation {
				Relation::None => {
					actions.child(parts::small_button("Add Friend", Look::Secondary, move || {
						friend.request(Request::Command(Command::AddFriend {
							username: username.clone(),
							from: FriendSource::Profile,
						}));
					}))
				}
				Relation::Outgoing => {
					actions.child(parts::small_button("Request Sent", Look::Disabled, || {}))
				}
				Relation::Incoming => {
					actions.child(parts::small_button("Accept", Look::Secondary, move || {
						friend.request(Request::Command(Command::AcceptFriend { user_id: id }));
					}))
				}
				Relation::Friend | Relation::Blocked => actions,
			};
		}
		if card.relation != Relation::Blocked {
			let block = layer.clone();
			actions = actions.child(parts::small_button("Block", Look::Ghost, move || {
				block.request(Request::Command(Command::BlockUser { user_id: id }));
			}));
		}
	}
	actions.child(parts::small_button("Copy ID", Look::Ghost, move || {
		crate::toasts::copy(&id.to_string(), "ID copied");
	}))
}
