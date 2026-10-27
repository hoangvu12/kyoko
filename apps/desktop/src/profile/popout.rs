use std::rc::Rc;

use bridge::{InviteTarget, ProfileCard, ProfileMenuItem, Relation, ToastKind};
use protocol::Snowflake;
use session::{Command, FriendSource};
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::input::{EventPhase, InputEvent, Key};

use super::parts::{self, Badge};
use super::{ProfileLayer, Request, Shown, Surface};
use crate::kit::{id_badge, menu_icon, menu_panel, menu_row, separator};
use crate::timeline::icons::icon;
use crate::timeline::tok;

const INVITE_MENU_HEIGHT: f32 = 400.;

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
	let ring = parts::ring_fill(card.theme, "bg_3");
	let badge = Badge::of(&layer.runtime, card.status, "_lg", 0.);
	parts::surface(
		column()
			.id("profile-popout")
			.relative()
			.w(tok("popout_width"))
			.rounded(tok("radius_lg"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.overflow_hidden()
			.text_color(tok("text_3")),
		card.theme,
		"bg_3",
	)
	.on_event(|event| {
		if event.phase != EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. }) {
			event.stop_propagation();
		}
	})
	.child(parts::banner(
		&pictures,
		card.accent,
		images.banner.as_ref(),
		"banner_height",
	))
	.when(card.effect, |view| {
		view.child(parts::effect_glow(&layer.runtime, card.theme, "banner_height"))
	})
	.child(banner_buttons(layer, shown))
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
				ring,
				parts::avatar_on(
					&pictures,
					images.avatar.as_ref(),
					parts::size(&layer.runtime, "avatar_size_lg"),
					Some(badge),
					ring,
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
	parts::inner(
		column()
			.w_full()
			.gap(tok("space_12"))
			.p(tok("space_12"))
			.rounded(tok("profile_inner_radius")),
		card.theme,
	)
	.children(top)
	.when(split, |card| card.child(parts::divider()))
	.children(below)
}

fn message_field(layer: &Rc<ProfileLayer>, card: &ProfileCard) -> View {
	let draft = layer.runtime.signal(String::new());
	let (sender, typed) = (layer.clone(), draft.clone());
	let id = card.id;
	let placeholder = format!("Message @{}", card.name);
	parts::inner(
		row()
			.id("profile-popout-message")
			.w_full()
			.h(tok("input_height"))
			.items_center()
			.gap(tok("space_8"))
			.pr(tok("space_12"))
			.rounded(tok("radius_sm"))
			.border(tok("border_thickness"))
			.border_color(tok("border_light"))
			.hover(|style| style.border_color(tok("border"))),
		card.theme,
	)
	.child(
		text_input(placeholder.clone(), draft)
			.id("profile-popout-message-input")
			.placeholder(placeholder)
			.grow()
			.min_w(0.)
			.h_full()
			.pl(tok("space_12"))
			.bg(crate::kit::clear())
			.border(0.)
			.text_color(tok("text_3"))
			.text_size(tok("font_size_sm"))
			.focus(|style| style.outline_width(0.))
			.on_event(move |event| {
				if !matches!(event.event, InputEvent::KeyDown { key: Key::Enter, .. }) {
					return;
				}
				event.prevent_default();
				let content = typed.with_untracked(|text| text.trim().to_owned());
				if content.is_empty() {
					return;
				}
				typed.set(String::new());
				sender.close_profile();
				sender.request(Request::QuickMessage(id, content));
			}),
	)
	.child(icon("smile", 20., "text_4"))
}

fn banner_buttons(layer: &Rc<ProfileLayer>, shown: &Shown) -> View {
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
			friend_button(layer, &shown.card, |glyph, caption, action| {
				parts::round_button("profile-popout-friend", glyph, caption, action)
			}),
			|line, button| line.child(button),
		)
		.child(more_menu(layer, shown, |toggle| {
			parts::round_button("profile-popout-more", "more", "More", toggle)
		}))
}

pub fn friend_button(
	layer: &Rc<ProfileLayer>,
	card: &ProfileCard,
	draw: impl Fn(&'static str, &str, Box<dyn FnMut()>) -> View,
) -> Option<View> {
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
	shown: &Shown,
	anchor: impl FnOnce(Box<dyn FnMut()>) -> View,
) -> View {
	let open = layer.runtime.signal(false);
	let toggle = open.clone();
	let anchor = anchor(Box::new(move || {
		toggle.set(!toggle.with_untracked(|open| *open));
	}));
	menu_panel(menu("Profile menu", open, anchor).placement(Placement {
		side: PlacementSide::Bottom,
		align: PlacementEdge::End,
		offset: 4.,
	}))
	.children(menu_items(layer, shown))
}

pub fn floating_menu(layer: &Rc<ProfileLayer>, shown: &Shown, at: (f32, f32)) -> View {
	let open = layer.runtime.signal(true);
	let watched = open.clone();
	let closer = layer.clone();
	let serial = shown.serial;
	let effect = layer.runtime.effect(move || {
		let showing = closer
			.surface
			.with_untracked(|surface| matches!(surface, Some(Surface::Menu { .. })))
			&& closer
				.shown
				.with_untracked(|current| current.as_ref().is_some_and(|current| current.serial == serial));
		if !watched.get() && showing {
			closer.close_profile();
		}
	});
	layer.effects.borrow_mut().push(effect);
	let spot = div().absolute().left(at.0).top(at.1).w(0.).h(0.);
	menu_panel(menu("Profile menu", open, spot).placement(Placement {
		side: PlacementSide::Bottom,
		align: PlacementEdge::End,
		offset: 4.,
	}))
	.id("profile-floating-menu")
	.children(menu_items(layer, shown))
}

fn menu_items(layer: &Rc<ProfileLayer>, shown: &Shown) -> Vec<View> {
	let mut views = Vec::new();
	for (index, group) in shown.menu.iter().enumerate() {
		if index > 0 {
			views.push(separator());
		}
		views.extend(group.iter().map(|item| menu_item(layer, shown.card.id, item)));
	}
	views
}

fn menu_item(layer: &Rc<ProfileLayer>, id: Snowflake, item: &ProfileMenuItem) -> View {
	if let ProfileMenuItem::InviteToServer(targets) = item {
		return invite_menu(layer, id, targets);
	}
	let tone = if item.is_danger() { "red_2" } else { "text_3" };
	let glyph = match item {
		ProfileMenuItem::CopyId => id_badge(),
		ProfileMenuItem::ViewFullProfile => menu_icon("profile", false),
		ProfileMenuItem::ShareProfile => menu_icon("link", false),
		ProfileMenuItem::Ignore | ProfileMenuItem::Unignore => menu_icon("eye", false),
		ProfileMenuItem::Block | ProfileMenuItem::Unblock => menu_icon("user_x", item.is_danger()),
		ProfileMenuItem::Report | ProfileMenuItem::InviteToServer(_) => menu_icon("flag", true),
	};
	let (this, chosen) = (layer.clone(), item.clone());
	menu_row(item.label().to_owned(), glyph, tone)
		.id(format!("profile-menu-{}", item.key()))
		.on_click(move || choose(&this, id, &chosen))
}

fn choose(layer: &Rc<ProfileLayer>, id: Snowflake, item: &ProfileMenuItem) {
	match item {
		ProfileMenuItem::ViewFullProfile => layer.request(Request::FullProfile(id)),
		ProfileMenuItem::ShareProfile => crate::toasts::copy(&bridge::profile_link(id), "Link copied"),
		ProfileMenuItem::CopyId => crate::toasts::copy(&id.to_string(), "User ID copied"),
		ProfileMenuItem::Ignore => {
			layer.request(Request::Command(Command::IgnoreUser { user_id: id }));
			crate::toasts::show(ToastKind::Success, "User ignored");
		}
		ProfileMenuItem::Unignore => {
			layer.request(Request::Command(Command::UnignoreUser { user_id: id }));
			crate::toasts::show(ToastKind::Success, "User unignored");
		}
		ProfileMenuItem::Block => {
			layer.close_profile();
			layer.request(Request::Block(id));
		}
		ProfileMenuItem::Unblock => {
			layer.request(Request::Command(Command::RemoveRelationship { user_id: id }));
		}
		ProfileMenuItem::Report => layer.request(Request::Report),
		ProfileMenuItem::InviteToServer(_) => {}
	}
}

fn invite_menu(layer: &Rc<ProfileLayer>, user_id: Snowflake, targets: &[InviteTarget]) -> View {
	let trigger = menu_row(
		"Invite to Server".to_owned(),
		menu_icon("chevron_right", false),
		"text_3",
	);
	let choices: Vec<View> = targets
		.iter()
		.map(|target| {
			let (this, channel_id) = (layer.clone(), target.channel_id);
			let sent = format!("Invite to {} sent", target.name);
			menu_row(target.name.clone(), div(), "text_3")
				.id(format!("profile-invite-{}", target.guild_id))
				.on_click(move || {
					this.request(Request::Command(Command::InviteToServer { user_id, channel_id }));
					crate::toasts::show(ToastKind::Success, &sent);
				})
		})
		.collect();
	menu_panel(submenu("Invite to Server", layer.runtime.signal(false)).trigger(trigger))
		.trigger_id("profile-menu-invite")
		.max_h(INVITE_MENU_HEIGHT)
		.children(choices)
}
