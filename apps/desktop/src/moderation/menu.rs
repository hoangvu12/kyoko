use std::rc::Rc;

use bridge::ModerationItem;
use session::Command;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::Signal;

use super::kit::{MENU_BORDER, MENU_ICON, MENU_ITEM, MENU_PADDING, SEPARATOR, label, menu_panel, separator};
use super::{MemberMenu, ModerationLayer, Request};
use crate::timeline::icons::icon;
use crate::timeline::tok;

const HEAD_ITEMS: usize = 3;
const ROLE_LIMIT: usize = 30;

pub fn size(menu: &MemberMenu) -> (f32, f32) {
	let mut rows = 0.;
	if menu.head {
		rows += HEAD_ITEMS as f32 * MENU_ITEM;
		if menu.developer {
			rows += SEPARATOR + MENU_ITEM;
		}
		if !menu.items.is_empty() {
			rows += SEPARATOR;
		}
	}
	for item in &menu.items {
		rows += match item {
			ModerationItem::Separator => SEPARATOR,
			_ => MENU_ITEM,
		};
	}
	(super::kit::MENU_WIDTH, rows + 2. * (MENU_PADDING + MENU_BORDER))
}

fn entry(name: &str, glyph: &'static str, color: &'static str) -> View {
	menu_item(name.to_owned())
		.h(MENU_ITEM)
		.px(12.)
		.py(0.)
		.rounded(tok("radius_sm"))
		.child(
			row()
				.items_center()
				.gap(5.)
				.child(icon(glyph, MENU_ICON, color))
				.child(label(name.to_owned(), "font_sm").text_color(tok(color))),
		)
}

pub fn view(
	layer: &Rc<ModerationLayer>,
	menu: &Rc<MemberMenu>,
	(x, y): (f32, f32),
	open: Signal<bool>,
) -> View {
	let spot = div().absolute().left(x).top(y).w(0.).h(0.);
	let mut items = Vec::new();
	if menu.head {
		let (profile, message) = (layer.clone(), layer.clone());
		let user_id = menu.user_id;
		items.push(
			entry("Profile", "profile", "text_normal")
				.on_click(move || profile.request(Request::Profile(user_id, (x, y)))),
		);
		items.push(
			entry("Message", "chat", "text_normal")
				.on_click(move || message.request(Request::Message(user_id))),
		);
		items.push(entry("Add Friend", "friend_requests", "text_muted").disabled(true));
		if menu.developer {
			let copy = layer.clone();
			items.push(separator());
			items.push(
				entry("Copy User ID", "copy", "text_normal")
					.on_click(move || copy.request(Request::CopyId(user_id))),
			);
		}
		if !menu.items.is_empty() {
			items.push(separator());
		}
	}
	for item in &menu.items {
		items.push(moderation_item(layer, menu, item));
	}
	menu_panel(
		zgx::compose::menu("Member menu", open, spot).placement(Placement {
			side: PlacementSide::Right,
			align: PlacementEdge::Start,
			offset: 0.,
		}),
	)
	.children(items)
}

fn moderation_item(layer: &Rc<ModerationLayer>, menu: &Rc<MemberMenu>, item: &ModerationItem) -> View {
	let color = if item.is_danger() { "danger" } else { "text_normal" };
	let (guild_id, user_id) = (menu.guild_id, menu.user_id);
	match item {
		ModerationItem::Separator => separator(),
		ModerationItem::Roles(roles) => {
			let choices: Vec<View> = roles
				.iter()
				.take(ROLE_LIMIT)
				.map(|role| {
					let (toggle, held, role_id) = (layer.clone(), role.held, role.id);
					menu_item(role.name.clone())
						.h(MENU_ITEM)
						.px(12.)
						.py(0.)
						.rounded(tok("radius_sm"))
						.menu_checked(held)
						.child(label(role.name.clone(), "font_sm").text_color(tok("text_normal")))
						.on_click(move || {
							let command = if held {
								Command::RemoveMemberRole {
									guild_id,
									user_id,
									role_id,
								}
							} else {
								Command::AddMemberRole {
									guild_id,
									user_id,
									role_id,
								}
							};
							toggle.request(Request::Command(command));
						})
				})
				.collect();
			submenu_of(layer, item, color, choices)
		}
		ModerationItem::MoveTo(channels) => {
			let mut choices: Vec<View> = channels
				.iter()
				.map(|channel| {
					let (mover, channel_id) = (layer.clone(), channel.id);
					menu_item(channel.name.clone())
						.h(MENU_ITEM)
						.px(12.)
						.py(0.)
						.rounded(tok("radius_sm"))
						.menu_checked(channel.current)
						.child(label(channel.name.clone(), "font_sm").text_color(tok("text_normal")))
						.on_click(move || {
							mover.request(Request::Command(Command::MoveMember {
								guild_id,
								user_id,
								channel_id: Some(channel_id),
							}));
						})
				})
				.collect();
			let disconnect = layer.clone();
			choices.push(separator());
			choices.push(entry("Disconnect", "phone_off", "text_normal").on_click(move || {
				disconnect.request(Request::Command(Command::MoveMember {
					guild_id,
					user_id,
					channel_id: None,
				}));
			}));
			submenu_of(layer, item, color, choices)
		}
		_ => {
			let (this, menu, item_value) = (layer.clone(), menu.clone(), item.clone());
			entry(item.label(), item.icon(), color).on_click(move || this.run_item(&menu, &item_value))
		}
	}
}

fn submenu_of(
	layer: &Rc<ModerationLayer>,
	item: &ModerationItem,
	color: &'static str,
	choices: Vec<View>,
) -> View {
	menu_panel(
		submenu(item.label(), layer.runtime.signal(false)).trigger(
			entry(item.label(), item.icon(), color).child(div().grow().child(icon(
				"chevron_right",
				MENU_ICON,
				"interactive_normal",
			))),
		),
	)
	.children(choices)
}
