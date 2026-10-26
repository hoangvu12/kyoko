use std::rc::Rc;

use bridge::ModerationItem;
use session::Command;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::Signal;

use super::kit::{menu_panel, separator};
use super::{MemberMenu, ModerationLayer, Request};
use crate::kit::{self, menu_icon, menu_row};

const HEAD_ITEMS: usize = 3;
const ROLE_LIMIT: usize = 30;

pub fn size(menu: &MemberMenu) -> (f32, f32) {
	let (mut rows, mut separators) = (0, 0);
	if menu.head {
		rows += HEAD_ITEMS;
		if menu.developer {
			rows += 1;
			separators += 1;
		}
		if !menu.items.is_empty() {
			separators += 1;
		}
	}
	for item in &menu.items {
		match item {
			ModerationItem::Separator => separators += 1,
			_ => rows += 1,
		}
	}
	(kit::MENU_WIDTH, kit::menu_height(rows, separators, 0.))
}

fn entry(name: &str, glyph: &'static str, color: &'static str) -> View {
	menu_row(name.to_owned(), menu_icon(glyph, color == "danger"), color)
}

fn choice(name: &str, checked: bool) -> View {
	menu_row(name.to_owned(), div(), "text_3").menu_checked(checked)
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
			entry("Profile", "profile", "text_3")
				.on_click(move || profile.request(Request::Profile(user_id, (x, y)))),
		);
		items.push(
			entry("Message", "chat", "text_3").on_click(move || message.request(Request::Message(user_id))),
		);
		items.push(entry("Add Friend", "friend_requests", "text_5").disabled(true));
		if menu.developer {
			let copy = layer.clone();
			items.push(separator());
			items.push(
				entry("Copy User ID", "copy", "text_3")
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
	let color = if item.is_danger() { "danger" } else { "text_3" };
	let (guild_id, user_id) = (menu.guild_id, menu.user_id);
	match item {
		ModerationItem::Separator => separator(),
		ModerationItem::Roles(roles) => {
			let choices: Vec<View> = roles
				.iter()
				.take(ROLE_LIMIT)
				.map(|role| {
					let (toggle, held, role_id) = (layer.clone(), role.held, role.id);
					choice(&role.name, held).on_click(move || {
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
					choice(&channel.name, channel.current).on_click(move || {
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
			choices.push(entry("Disconnect", "phone_off", "text_3").on_click(move || {
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
		submenu(item.label(), layer.runtime.signal(false)).trigger(menu_row(
			item.label().to_owned(),
			menu_icon("chevron_right", false),
			color,
		)),
	)
	.children(choices)
}
