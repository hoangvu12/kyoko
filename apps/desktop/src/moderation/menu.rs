use std::rc::Rc;

use bridge::ModerationItem;
use session::Command;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::Signal;
use zgx::scene::Direction;

use super::kit::{menu_panel, separator};
use super::{MemberMenu, ModerationLayer, Request};
use crate::kit::{self, menu_icon, menu_row};

const HEAD_ITEMS: usize = 3;
const ROLE_LIMIT: usize = 30;

#[derive(Debug, Clone, Copy, Default)]
struct Count {
	rows: usize,
	separators: usize,
}

impl Count {
	fn add(&mut self, item: &ModerationItem) {
		match item {
			ModerationItem::Separator => self.separators += 1,
			_ => self.rows += 1,
		}
	}

	fn offset(self) -> f32 {
		self.rows as f32 * kit::MENU_ITEM
			+ self.separators as f32 * kit::SEPARATOR
			+ (self.rows + self.separators) as f32 * kit::MENU_GAP
	}
}

fn head(menu: &MemberMenu) -> Count {
	let mut count = Count::default();
	if menu.head {
		count.rows += HEAD_ITEMS;
		if menu.developer {
			count.rows += 1;
			count.separators += 1;
		}
		if !menu.items.is_empty() {
			count.separators += 1;
		}
	}
	count
}

pub fn size(menu: &MemberMenu) -> (f32, f32) {
	let mut count = head(menu);
	for item in &menu.items {
		count.add(item);
	}
	(
		kit::MENU_WIDTH,
		kit::menu_height(count.rows, count.separators, 0.),
	)
}

pub fn roles_center(menu: &MemberMenu, (x, y): (f32, f32)) -> Option<(f32, f32)> {
	let mut count = head(menu);
	for item in &menu.items {
		if matches!(item, ModerationItem::Roles(_)) {
			let top = y + kit::MENU_BORDER + kit::MENU_PADDING + count.offset();
			return Some((x + kit::MENU_WIDTH / 2., top + kit::MENU_ITEM / 2.));
		}
		count.add(item);
	}
	None
}

#[derive(Debug, Clone, Copy)]
struct Room {
	left: bool,
	below: f32,
}

fn entry(name: &str, glyph: &'static str, color: &'static str) -> View {
	menu_row(name.to_owned(), menu_icon(glyph, color == "danger"), color)
}

fn choice(name: &str, mark: View, checked: bool) -> View {
	menu_row(name.to_owned(), mark, "text_3").menu_checked(checked)
}

pub fn view(
	layer: &Rc<ModerationLayer>,
	menu: &Rc<MemberMenu>,
	(x, y): (f32, f32),
	open: Signal<bool>,
) -> View {
	let spot = div().absolute().left(x).top(y).w(0.).h(0.);
	let (width, height) = layer.window.with_untracked(|window| *window);
	let left = x + 2. * kit::MENU_WIDTH > width - kit::MENU_MARGIN;
	let first = y + kit::MENU_BORDER + kit::MENU_PADDING;
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
	let mut count = head(menu);
	for item in &menu.items {
		let room = Room {
			left,
			below: height - kit::MENU_MARGIN - first - count.offset(),
		};
		items.push(moderation_item(layer, menu, item, room));
		count.add(item);
	}
	menu_panel(
		zgx::compose::menu("Member menu", open, spot).placement(Placement {
			side: PlacementSide::Right,
			align: PlacementEdge::Start,
			offset: 0.,
		}),
	)
	.id("moderation-member-menu")
	.children(items)
}

fn moderation_item(
	layer: &Rc<ModerationLayer>,
	menu: &Rc<MemberMenu>,
	item: &ModerationItem,
	room: Room,
) -> View {
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
					choice(&role.name, kit::checkbox(move || held), held)
						.id(format!("moderation-role-{role_id}"))
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
			submenu_of(layer, item, color, choices, room)
				.id("moderation-roles-menu")
				.trigger_id("moderation-roles")
		}
		ModerationItem::MoveTo(channels) => {
			let mut choices: Vec<View> = channels
				.iter()
				.map(|channel| {
					let (mover, channel_id) = (layer.clone(), channel.id);
					let mark = if channel.current {
						menu_icon("check", false)
					} else {
						div()
					};
					choice(&channel.name, mark, channel.current).on_click(move || {
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
			submenu_of(layer, item, color, choices, room)
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
	room: Room,
) -> View {
	let trigger = menu_row(item.label().to_owned(), menu_icon("chevron_right", false), color);
	let direction = if room.left { Direction::Rtl } else { Direction::Ltr };
	menu_panel(submenu(item.label(), layer.runtime.signal(false)).trigger(trigger.direction(direction)))
		.direction(Direction::Ltr)
		.max_h(room.below)
		.children(choices)
}
