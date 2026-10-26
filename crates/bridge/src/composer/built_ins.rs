use std::collections::HashSet;

use protocol::{Gif, Snowflake};
use state::State;

use crate::sticker_picker::{StickerCell, StickerPicker};

const MAX_RESULTS: usize = 25;
const SEARCHING: &str = "Searching...";
const NO_GIFS: &str = "No GIFs found. Try another search.";
const NO_STICKERS: &str = "No stickers found.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moderation {
	Ban,
	Kick,
	Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationAsk {
	pub action: Moderation,
	pub guild_id: Snowflake,
	pub user_id: Snowflake,
	pub name: String,
	pub seconds: Option<i64>,
	pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
	Gif(Gif),
	Sticker(Box<StickerCell>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResults {
	pub items: Vec<CommandResult>,
	pub selected: usize,
	pub status: Option<&'static str>,
}

impl CommandResults {
	pub(super) fn of(items: Vec<CommandResult>, selected: usize, empty: &'static str) -> Self {
		let status = items.is_empty().then_some(empty);
		let selected = selected.min(items.len().saturating_sub(1));
		Self {
			items,
			selected,
			status,
		}
	}

	pub(super) fn searching() -> Self {
		Self {
			items: Vec::new(),
			selected: 0,
			status: Some(SEARCHING),
		}
	}

	pub fn selected_item(&self) -> Option<&CommandResult> {
		self.items.get(self.selected)
	}
}

pub(super) fn normalized(text: &str) -> String {
	text.trim().to_lowercase()
}

pub(super) fn gif_results(state: &State, query: &str, selected: usize) -> Option<CommandResults> {
	let query = normalized(query);
	if query.is_empty() {
		return None;
	}
	Some(match state.gif_search(&query) {
		Some(gifs) => CommandResults::of(
			gifs.iter()
				.take(MAX_RESULTS)
				.cloned()
				.map(CommandResult::Gif)
				.collect(),
			selected,
			NO_GIFS,
		),
		None => CommandResults::searching(),
	})
}

pub(super) fn sticker_results(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	query: &str,
	selected: usize,
) -> CommandResults {
	let picker = StickerPicker::of(state, guild_id, channel_id);
	let found: Vec<StickerCell> = if query.trim().is_empty() {
		picker
			.sections()
			.iter()
			.flat_map(|section| section.stickers.iter().cloned())
			.collect()
	} else {
		picker.search(query)
	};
	let mut seen = HashSet::new();
	let items = found
		.into_iter()
		.filter(|cell| !cell.locked && seen.insert(cell.sticker.id))
		.take(MAX_RESULTS)
		.map(|cell| CommandResult::Sticker(Box::new(cell)))
		.collect();
	CommandResults::of(items, selected, NO_STICKERS)
}

fn mention_or_id(raw: &str) -> Option<Snowflake> {
	let inner = raw
		.strip_prefix("<@")
		.and_then(|rest| rest.strip_suffix('>'))
		.map(|rest| rest.trim_start_matches('!'))
		.unwrap_or(raw);
	inner.parse::<u64>().ok().map(Snowflake)
}

fn known_users(state: &State, guild_id: Option<Snowflake>) -> Vec<Snowflake> {
	let mut ids: Vec<Snowflake> = guild_id
		.and_then(|guild_id| state.guild(guild_id))
		.map(|guild| guild.members.iter().map(|(id, _)| *id).collect())
		.unwrap_or_default();
	for channel in state.private_channels() {
		ids.extend(channel.recipient_ids.iter().copied());
		ids.extend(channel.recipients.iter().map(|user| user.id));
	}
	ids.extend(
		state
			.relationships()
			.filter_map(|relationship| relationship.user_id.or(Some(relationship.id))),
	);
	ids
}

pub(super) fn resolve_user(state: &State, guild_id: Option<Snowflake>, raw: &str) -> Option<Snowflake> {
	let raw = raw.trim();
	if let Some(id) = mention_or_id(raw) {
		return Some(id);
	}
	let wanted = raw.strip_prefix('@').unwrap_or(raw).to_lowercase();
	if wanted.is_empty() {
		return None;
	}
	known_users(state, guild_id).into_iter().find(|id| {
		let named = state.user(*id).is_some_and(|user| {
			user.username.to_lowercase() == wanted
				|| user
					.global_name
					.as_deref()
					.is_some_and(|name| name.to_lowercase() == wanted)
		});
		named || state.display_name(guild_id, *id).to_lowercase() == wanted
	})
}
