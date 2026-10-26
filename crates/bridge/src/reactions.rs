use std::sync::Arc;

use protocol::expressions::unicode::by_glyph;
use protocol::{Message, PartialEmoji, Reaction, Snowflake, User};
use state::State;

use crate::message::{person_name, reaction_key};

pub const REACTOR_PAGE: usize = 100;
const NAMED_IN_TIP: usize = 2;
const ALL_NAMED_UP_TO: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct ReactionChip {
	pub key: String,
	pub emoji: PartialEmoji,
	pub count: i64,
	pub burst: bool,
	pub me: bool,
	pub colors: Vec<u32>,
}

impl ReactionChip {
	pub fn id(&self) -> String {
		format!("{}-{}", if self.burst { "super" } else { "normal" }, self.key)
	}
}

pub fn hex_color(text: &str) -> Option<u32> {
	let digits = text.trim().strip_prefix('#')?;
	(digits.len() == 6)
		.then(|| u32::from_str_radix(digits, 16).ok())
		.flatten()
}

fn colors_of(reaction: &Reaction) -> Vec<u32> {
	reaction
		.burst_colors
		.iter()
		.filter_map(|color| hex_color(color))
		.collect()
}

pub fn reaction_chips(message: &Message) -> Vec<ReactionChip> {
	let mut chips = Vec::new();
	for reaction in message.reactions.iter() {
		let key = reaction_key(&reaction.emoji);
		let bursts = reaction.burst_count();
		if bursts > 0 {
			chips.push(ReactionChip {
				key: key.clone(),
				emoji: reaction.emoji.clone(),
				count: bursts,
				burst: true,
				me: reaction.me_burst,
				colors: colors_of(reaction),
			});
		}
		let normal = reaction.normal_count();
		if normal > 0 {
			chips.push(ReactionChip {
				key,
				emoji: reaction.emoji.clone(),
				count: normal,
				burst: false,
				me: reaction.me,
				colors: Vec::new(),
			});
		}
	}
	chips
}

pub fn emoji_label(emoji: &PartialEmoji) -> String {
	let name = emoji.name.as_deref().unwrap_or("emoji");
	if emoji.id.is_some() {
		return format!(":{name}:");
	}
	let Some(known) = by_glyph(name) else {
		return name.to_owned();
	};
	let word = known
		.names()
		.find(|candidate| candidate.starts_with(|first: char| first.is_ascii_alphabetic()))
		.unwrap_or(known.shortcode);
	format!(":{word}:")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionTip {
	pub who: String,
	pub label: String,
}

pub fn reaction_tip(names: Option<&[String]>, count: i64, burst: bool, label: String) -> ReactionTip {
	let verb = if burst {
		"super reacted with"
	} else {
		"reacted with"
	};
	let count = usize::try_from(count).unwrap_or(0);
	let who = match names {
		Some(names) if !names.is_empty() => people(names, count),
		_ if count == 1 => "1 person".to_owned(),
		_ => format!("{count} people"),
	};
	ReactionTip {
		who: format!("{who} {verb} "),
		label,
	}
}

fn people(names: &[String], count: usize) -> String {
	let count = count.max(names.len());
	let named = if count <= ALL_NAMED_UP_TO {
		&names[..names.len().min(count)]
	} else {
		&names[..names.len().min(NAMED_IN_TIP)]
	};
	let others = count - named.len();
	let mut parts: Vec<String> = named.to_vec();
	match others {
		0 => {}
		1 => parts.push("1 other".to_owned()),
		others => parts.push(format!("{others} others")),
	}
	match parts.as_slice() {
		[] => String::new(),
		[only] => only.clone(),
		[rest @ .., last] => format!("{} and {last}", rest.join(", ")),
	}
}

pub fn tip_names(
	state: &State,
	guild_id: Option<Snowflake>,
	message_id: Snowflake,
	chip: &ReactionChip,
) -> Option<Vec<String>> {
	let reactors = state.reactors(message_id, &chip.key, chip.burst)?;
	Some(
		reactors
			.users
			.iter()
			.take(ALL_NAMED_UP_TO)
			.map(|user| person_name(state, guild_id, user))
			.collect(),
	)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reactor {
	pub user: Arc<User>,
	pub name: String,
	pub username: String,
	pub removable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReactionsList {
	pub column: Vec<ReactionChip>,
	pub selected: Option<ReactionChip>,
	pub reactors: Vec<Reactor>,
	pub loaded: bool,
	pub has_more: bool,
	pub has_super: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRequest {
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub super_tab: bool,
	pub selected: Option<(String, bool)>,
	pub moderator: bool,
}

impl ReactionsList {
	pub fn of(state: &State, guild_id: Option<Snowflake>, request: &ListRequest) -> Option<Self> {
		let message = state
			.messages(request.channel_id)?
			.by_id
			.get(&request.message_id)?;
		let chips = reaction_chips(message);
		let has_super = chips.iter().any(|chip| chip.burst);
		let column: Vec<ReactionChip> = chips
			.into_iter()
			.filter(|chip| chip.burst || !request.super_tab)
			.collect();
		let selected = request
			.selected
			.as_ref()
			.and_then(|(key, burst)| {
				column
					.iter()
					.find(|chip| chip.key == *key && chip.burst == *burst)
			})
			.or_else(|| column.first())
			.cloned();
		let me = state.current_user_id();
		let reactors = selected
			.as_ref()
			.and_then(|chip| state.reactors(request.message_id, &chip.key, chip.burst));
		let list = reactors.map(|reactors| {
			reactors
				.users
				.iter()
				.map(|user| Reactor {
					user: user.clone(),
					name: person_name(state, guild_id, user),
					username: user.username.clone(),
					removable: request.moderator && Some(user.id) != me,
				})
				.collect()
		});
		Some(Self {
			column,
			selected,
			loaded: list.is_some(),
			has_more: reactors.is_some_and(|reactors| reactors.has_more),
			reactors: list.unwrap_or_default(),
			has_super,
		})
	}

	pub fn after(&self) -> Option<Snowflake> {
		self.reactors.iter().map(|reactor| reactor.user.id).max()
	}
}
