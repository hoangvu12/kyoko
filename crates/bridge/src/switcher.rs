use std::sync::Arc;

use protocol::{ChannelType, Snowflake, User};
use state::State;

use crate::sidebar::channel_icon;

pub const MAX_RESULTS: usize = 9;
pub const MAX_RECENT: usize = 5;
pub const RECENT_TARGETS: usize = 8;
const MATCH: i32 = 10;
const ADJACENT_BONUS: i32 = 15;
const WORD_START_BONUS: i32 = 20;
const LEADING_GAP_PENALTY: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwitcherTarget {
	Channel {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	},
	Guild(Snowflake),
	Friend(Snowflake),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwitcherCategory {
	Recent,
	Channels,
	Voice,
	DirectMessages,
	Friends,
	Servers,
}

impl SwitcherCategory {
	fn rank(self) -> u8 {
		match self {
			Self::Recent => 0,
			Self::Channels => 1,
			Self::Voice => 2,
			Self::DirectMessages => 3,
			Self::Friends => 4,
			Self::Servers => 5,
		}
	}

	pub fn title(self) -> &'static str {
		match self {
			Self::Recent => "RECENT",
			Self::Channels => "TEXT CHANNELS",
			Self::Voice => "VOICE CHANNELS",
			Self::DirectMessages => "DIRECT MESSAGES",
			Self::Friends => "FRIENDS",
			Self::Servers => "SERVERS",
		}
	}

	fn narrowed_by(self, prefix: char) -> bool {
		match prefix {
			'#' => self == Self::Channels,
			'!' => self == Self::Voice,
			'@' => matches!(self, Self::DirectMessages | Self::Friends),
			'*' => self == Self::Servers,
			_ => true,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum Leading {
	Icon(&'static str),
	User(Arc<User>),
	GuildIcon { guild_id: Snowflake, hash: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitcherEntry {
	pub target: SwitcherTarget,
	pub name: String,
	pub context: Option<String>,
	pub leading: Leading,
	pub category: SwitcherCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
	pub index: usize,
	pub category: SwitcherCategory,
}

pub fn fuzzy_score(query: &[char], name: &str) -> Option<i32> {
	let mut matched = 0;
	let mut score = 0;
	let mut previous: Option<usize> = None;
	let mut previous_char = ' ';
	for (at, character) in name.chars().enumerate() {
		if matched < query.len() && character.to_lowercase().eq(query[matched].to_lowercase()) {
			score += MATCH;
			if previous.is_some_and(|last| last + 1 == at) {
				score += ADJACENT_BONUS;
			}
			if !previous_char.is_alphanumeric() {
				score += WORD_START_BONUS;
			}
			if previous.is_none() {
				score -= i32::try_from(at).unwrap_or(i32::MAX) * LEADING_GAP_PENALTY;
			}
			previous = Some(at);
			matched += 1;
		}
		previous_char = character;
	}
	(matched == query.len()).then_some(score)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentTargets(Vec<SwitcherTarget>);

impl RecentTargets {
	pub fn remember(&mut self, target: SwitcherTarget) {
		self.0.retain(|recent| *recent != target);
		self.0.insert(0, target);
		self.0.truncate(RECENT_TARGETS);
	}

	pub fn targets(&self) -> &[SwitcherTarget] {
		&self.0
	}
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuickSwitcher {
	entries: Vec<SwitcherEntry>,
	recent: Vec<SwitcherTarget>,
	query: String,
	selected: usize,
}

impl QuickSwitcher {
	pub fn new(entries: Vec<SwitcherEntry>, recent: &RecentTargets) -> Self {
		Self {
			entries,
			recent: recent.targets().to_vec(),
			query: String::new(),
			selected: 0,
		}
	}

	pub fn of(state: &State, recent: &RecentTargets) -> Self {
		Self::new(switcher_entries(state), recent)
	}

	pub fn entries(&self) -> &[SwitcherEntry] {
		&self.entries
	}

	pub fn entry(&self, shown: &Shown) -> &SwitcherEntry {
		&self.entries[shown.index]
	}

	pub fn query(&self) -> &str {
		&self.query
	}

	pub fn selected(&self) -> usize {
		self.selected
	}

	pub fn set_query(&mut self, query: &str) {
		if self.query != query {
			query.clone_into(&mut self.query);
			self.selected = 0;
		}
	}

	pub fn select(&mut self, position: usize) {
		if position < self.results().len() {
			self.selected = position;
		}
	}

	pub fn step(&mut self, forward: bool) {
		let count = self.results().len();
		if count == 0 {
			return;
		}
		self.selected = if forward {
			(self.selected + 1) % count
		} else {
			(self.selected + count - 1) % count
		};
	}

	pub fn picked(&self) -> Option<SwitcherTarget> {
		self.results()
			.get(self.selected)
			.map(|shown| self.entries[shown.index].target)
	}

	fn recency(&self, target: SwitcherTarget) -> usize {
		self.recent
			.iter()
			.position(|recent| *recent == target)
			.unwrap_or(usize::MAX)
	}

	pub fn results(&self) -> Vec<Shown> {
		let trimmed = self.query.trim_start();
		let prefix = trimmed
			.chars()
			.next()
			.filter(|first| matches!(first, '#' | '!' | '@' | '*'));
		let text = prefix
			.map_or(trimmed, |prefix| &trimmed[prefix.len_utf8()..])
			.trim();
		if text.is_empty() && prefix.is_none() {
			return self.recent_results();
		}
		let needle: Vec<char> = text.chars().collect();
		let mut scored: Vec<(i32, usize, usize)> = self
			.entries
			.iter()
			.enumerate()
			.filter(|(_, entry)| prefix.is_none_or(|prefix| entry.category.narrowed_by(prefix)))
			.filter_map(|(index, entry)| {
				let score = if needle.is_empty() {
					0
				} else {
					fuzzy_score(&needle, &entry.name)?
				};
				Some((score, self.recency(entry.target), index))
			})
			.collect();
		scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
		scored.truncate(MAX_RESULTS);
		let mut shown: Vec<Shown> = scored
			.into_iter()
			.map(|(_, _, index)| Shown {
				index,
				category: self.entries[index].category,
			})
			.collect();
		shown.sort_by_key(|item| item.category.rank());
		shown
	}

	fn recent_results(&self) -> Vec<Shown> {
		let mut shown: Vec<Shown> = self
			.recent
			.iter()
			.filter_map(|target| self.entries.iter().position(|entry| entry.target == *target))
			.take(MAX_RECENT)
			.map(|index| Shown {
				index,
				category: SwitcherCategory::Recent,
			})
			.collect();
		let taken: Vec<usize> = shown.iter().map(|item| item.index).collect();
		let mut rest: Vec<Shown> = self
			.entries
			.iter()
			.enumerate()
			.filter(|(index, _)| !taken.contains(index))
			.take(MAX_RESULTS - shown.len())
			.map(|(index, entry)| Shown {
				index,
				category: entry.category,
			})
			.collect();
		rest.sort_by_key(|item| item.category.rank());
		shown.extend(rest);
		shown
	}
}

fn opens(kind: ChannelType) -> bool {
	!matches!(
		kind,
		ChannelType::GuildForum | ChannelType::GuildMedia | ChannelType::GuildCategory
	)
}

pub fn switcher_entries(state: &State) -> Vec<SwitcherEntry> {
	let mut entries = Vec::new();
	let guilds = state.guild_summaries();
	for summary in &guilds {
		for node in state.guild_channel_tree(summary.id) {
			for channel in node
				.children
				.iter()
				.filter(|channel| !channel.locked && opens(channel.kind))
			{
				let voice = matches!(
					channel.kind,
					ChannelType::GuildVoice | ChannelType::GuildStageVoice
				);
				entries.push(SwitcherEntry {
					target: SwitcherTarget::Channel {
						guild_id: Some(summary.id),
						channel_id: channel.id,
					},
					name: channel.name.clone(),
					context: Some(summary.name.clone()),
					leading: Leading::Icon(channel_icon(channel.kind)),
					category: if voice {
						SwitcherCategory::Voice
					} else {
						SwitcherCategory::Channels
					},
				});
			}
		}
	}
	let rows = state.dm_rows();
	let dm_users: Vec<Snowflake> = rows
		.iter()
		.filter(|row| row.recipients.len() == 1)
		.map(|row| row.recipients[0])
		.collect();
	entries.extend(rows.into_iter().map(|row| {
		let leading = match row.recipients.as_slice() {
			[user] => state
				.user(*user)
				.map_or(Leading::Icon("at_sign"), |user| Leading::User(user.clone())),
			_ => Leading::Icon("group"),
		};
		SwitcherEntry {
			target: SwitcherTarget::Channel {
				guild_id: None,
				channel_id: row.channel_id,
			},
			name: row.name,
			context: None,
			leading,
			category: SwitcherCategory::DirectMessages,
		}
	}));
	entries.extend(
		state
			.friends()
			.into_iter()
			.filter(|friend| !dm_users.contains(&friend.user_id))
			.map(|friend| SwitcherEntry {
				target: SwitcherTarget::Friend(friend.user_id),
				leading: state
					.user(friend.user_id)
					.map_or(Leading::Icon("at_sign"), |user| Leading::User(user.clone())),
				name: friend.display,
				context: Some(friend.username),
				category: SwitcherCategory::Friends,
			}),
	);
	entries.extend(guilds.into_iter().map(|summary| {
		SwitcherEntry {
			target: SwitcherTarget::Guild(summary.id),
			leading: summary
				.icon
				.map_or(Leading::Icon("home"), |hash| Leading::GuildIcon {
					guild_id: summary.id,
					hash,
				}),
			name: summary.name,
			context: None,
			category: SwitcherCategory::Servers,
		}
	}));
	entries
}
