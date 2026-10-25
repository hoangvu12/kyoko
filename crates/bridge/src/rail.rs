use std::collections::HashMap;

use protocol::Snowflake;
use session::Command;
use state::{Changes, GuildSummary, RailEntry, State};

pub const DEFAULT_FOLDER_COLOR: u32 = 0x5865f2;
const FOLDER_NAME_GUILDS: usize = 3;
const ACRONYM_WORDS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailGuild {
	pub id: Snowflake,
	pub name: String,
	pub acronym: String,
	pub icon: Option<String>,
	pub unread: bool,
	pub mentions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailFolder {
	pub id: u64,
	pub name: String,
	pub color: u32,
	pub guilds: Vec<RailGuild>,
}

impl RailFolder {
	pub fn unread(&self) -> bool {
		self.guilds.iter().any(|guild| guild.unread)
	}

	pub fn mentions(&self) -> u32 {
		self.guilds.iter().map(|guild| guild.mentions).sum()
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RailItem {
	Guild(RailGuild),
	Folder(RailFolder),
}

#[derive(Debug, Default)]
pub struct Rail {
	items: Vec<RailItem>,
	home_mentions: u32,
}

impl Rail {
	pub fn items(&self) -> &[RailItem] {
		&self.items
	}

	pub fn home_mentions(&self) -> u32 {
		self.home_mentions
	}

	pub fn guilds(&self) -> impl Iterator<Item = &RailGuild> {
		self.items.iter().flat_map(|item| match item {
			RailItem::Guild(guild) => std::slice::from_ref(guild).iter(),
			RailItem::Folder(folder) => folder.guilds.iter(),
		})
	}

	pub fn folder_of(&self, guild_id: Snowflake) -> Option<u64> {
		self.items.iter().find_map(|item| match item {
			RailItem::Folder(folder) if folder.guilds.iter().any(|guild| guild.id == guild_id) => {
				Some(folder.id)
			}
			_ => None,
		})
	}

	pub fn apply(&mut self, state: &State, changes: &Changes) -> bool {
		let relevant =
			changes.guild_list || changes.read_state || changes.user_settings || changes.private_channels;
		if !relevant {
			return false;
		}
		let items = rail_items(state);
		let home_mentions = state.dm_rows().iter().map(|row| row.mentions).sum();
		if items == self.items && home_mentions == self.home_mentions {
			return false;
		}
		self.items = items;
		self.home_mentions = home_mentions;
		true
	}
}

pub fn acronym(name: &str) -> String {
	name.split_whitespace()
		.filter_map(|word| word.chars().next())
		.take(ACRONYM_WORDS)
		.flat_map(char::to_uppercase)
		.collect()
}

fn rail_guild(summary: &GuildSummary) -> RailGuild {
	RailGuild {
		id: summary.id,
		name: summary.name.clone(),
		acronym: acronym(&summary.name),
		icon: summary.icon.clone(),
		unread: summary.unread && !summary.muted,
		mentions: summary.mentions,
	}
}

fn rail_items(state: &State) -> Vec<RailItem> {
	let summaries = state.guild_summaries();
	let by_id: HashMap<Snowflake, &GuildSummary> =
		summaries.iter().map(|summary| (summary.id, summary)).collect();
	state
		.guild_layout()
		.into_iter()
		.filter_map(|entry| match entry {
			RailEntry::Guild(id) => by_id.get(&id).map(|summary| RailItem::Guild(rail_guild(summary))),
			RailEntry::Folder {
				id,
				name,
				color,
				guilds,
			} => {
				let guilds: Vec<RailGuild> = guilds
					.iter()
					.filter_map(|id| by_id.get(id))
					.map(|summary| rail_guild(summary))
					.collect();
				if guilds.is_empty() {
					return None;
				}
				let name = name.filter(|name| !name.is_empty()).unwrap_or_else(|| {
					guilds
						.iter()
						.take(FOLDER_NAME_GUILDS)
						.map(|guild| guild.name.as_str())
						.collect::<Vec<_>>()
						.join(", ")
				});
				Some(RailItem::Folder(RailFolder {
					id,
					name,
					color: color.unwrap_or(DEFAULT_FOLDER_COLOR),
					guilds,
				}))
			}
		})
		.collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
	BeforeGuild(Snowflake),
	BeforeFolder(u64),
	End,
	OntoGuild(Snowflake),
	OntoFolder(u64),
}

fn remove_guild(layout: &mut Vec<RailEntry>, moved: Snowflake) {
	layout.retain_mut(|entry| match entry {
		RailEntry::Guild(id) => *id != moved,
		RailEntry::Folder { guilds, .. } => {
			guilds.retain(|id| *id != moved);
			!guilds.is_empty()
		}
	});
}

fn top_level_guild(layout: &[RailEntry], guild: Snowflake) -> Option<usize> {
	layout.iter().position(|entry| *entry == RailEntry::Guild(guild))
}

fn folder_guilds(layout: &mut [RailEntry], target: u64) -> Option<&mut Vec<Snowflake>> {
	layout.iter_mut().find_map(|entry| match entry {
		RailEntry::Folder { id, guilds, .. } if *id == target => Some(guilds),
		_ => None,
	})
}

fn insert_guild(layout: &mut Vec<RailEntry>, moved: Snowflake, drop: Drop, folder_id: u64) -> Option<()> {
	match drop {
		Drop::End => layout.push(RailEntry::Guild(moved)),
		Drop::BeforeFolder(target) => {
			let at = layout
				.iter()
				.position(|entry| matches!(entry, RailEntry::Folder { id, .. } if *id == target))?;
			layout.insert(at, RailEntry::Guild(moved));
		}
		Drop::BeforeGuild(target) => match top_level_guild(layout, target) {
			Some(at) => layout.insert(at, RailEntry::Guild(moved)),
			None => {
				let guilds = layout.iter_mut().find_map(|entry| match entry {
					RailEntry::Folder { guilds, .. } if guilds.contains(&target) => Some(guilds),
					_ => None,
				})?;
				let at = guilds.iter().position(|id| *id == target)?;
				guilds.insert(at, moved);
			}
		},
		Drop::OntoGuild(target) => match top_level_guild(layout, target) {
			Some(at) => {
				layout[at] = RailEntry::Folder {
					id: folder_id,
					name: None,
					color: None,
					guilds: vec![target, moved],
				};
			}
			None => return insert_guild(layout, moved, Drop::BeforeGuild(target), folder_id),
		},
		Drop::OntoFolder(target) => folder_guilds(layout, target)?.push(moved),
	}
	Some(())
}

pub fn rearranged(
	layout: &[RailEntry],
	moved: Snowflake,
	drop: Drop,
	folder_id: u64,
) -> Option<Vec<RailEntry>> {
	if drop == Drop::OntoGuild(moved) || drop == Drop::BeforeGuild(moved) {
		return None;
	}
	let known = layout.iter().any(|entry| match entry {
		RailEntry::Guild(id) => *id == moved,
		RailEntry::Folder { guilds, .. } => guilds.contains(&moved),
	});
	if !known {
		return None;
	}
	let mut next = layout.to_vec();
	remove_guild(&mut next, moved);
	insert_guild(&mut next, moved, drop, folder_id)?;
	(next != layout).then_some(next)
}

pub fn reorder_command(state: &State, moved: Snowflake, drop: Drop, folder_id: u64) -> Option<Command> {
	rearranged(&state.guild_layout(), moved, drop, folder_id).map(Command::SetGuildLayout)
}

fn top_level_index(layout: &[RailEntry], drop: Drop) -> Option<usize> {
	match drop {
		Drop::End => Some(layout.len()),
		Drop::BeforeFolder(target) => layout
			.iter()
			.position(|entry| matches!(entry, RailEntry::Folder { id, .. } if *id == target)),
		Drop::BeforeGuild(target) => layout.iter().position(|entry| match entry {
			RailEntry::Guild(id) => *id == target,
			RailEntry::Folder { guilds, .. } => guilds.contains(&target),
		}),
		Drop::OntoGuild(_) | Drop::OntoFolder(_) => None,
	}
}

pub fn rearranged_folder(layout: &[RailEntry], moved: u64, drop: Drop) -> Option<Vec<RailEntry>> {
	if drop == Drop::BeforeFolder(moved) {
		return None;
	}
	let from = layout
		.iter()
		.position(|entry| matches!(entry, RailEntry::Folder { id, .. } if *id == moved))?;
	let mut next = layout.to_vec();
	let folder = next.remove(from);
	let at = top_level_index(&next, drop)?;
	next.insert(at, folder);
	(next != layout).then_some(next)
}

pub fn reorder_folder_command(state: &State, moved: u64, drop: Drop) -> Option<Command> {
	rearranged_folder(&state.guild_layout(), moved, drop).map(Command::SetGuildLayout)
}

pub fn new_folder_id() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_or(1, |elapsed| elapsed.as_millis() as u64)
}
