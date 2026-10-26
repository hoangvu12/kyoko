use chrono::NaiveDate;
use protocol::Snowflake;
use state::{RailEntry, State};

use crate::clock::message_time;
use crate::message::{Span, snippet_spans};
use crate::rail::acronym;
use crate::sidebar::channel_icon;
use crate::timeline::detached_entry;

const MENTION_TEXT: usize = 300;
const ATTACHMENT_ONLY: &str = "Click to see attachment";
const FRIEND_REQUEST: &str = "sent you a friend request.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InboxTab {
	ForYou,
	Unreads,
	#[default]
	Mentions,
}

impl InboxTab {
	pub const ALL: [InboxTab; 3] = [InboxTab::ForYou, InboxTab::Unreads, InboxTab::Mentions];

	pub fn label(self) -> &'static str {
		match self {
			InboxTab::ForYou => "For You",
			InboxTab::Unreads => "Unreads",
			InboxTab::Mentions => "Mentions",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
	pub guild_id: Option<Snowflake>,
	pub guild: String,
	pub acronym: String,
	pub icon: Option<String>,
	pub channel_id: Snowflake,
	pub channel: String,
	pub channel_icon: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionCard {
	pub id: Snowflake,
	pub author_id: Snowflake,
	pub author: String,
	pub author_color: Option<u32>,
	pub time: String,
	pub spans: Vec<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionGroup {
	pub place: Place,
	pub cards: Vec<MentionCard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadRow {
	pub place: Place,
	pub mentions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadGroup {
	pub guild_id: Snowflake,
	pub guild: String,
	pub acronym: String,
	pub icon: Option<String>,
	pub rows: Vec<UnreadRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForYouEntry {
	pub user_id: Snowflake,
	pub name: String,
	pub text: &'static str,
	pub time: String,
}

pub fn place(state: &State, guild_id: Option<Snowflake>, channel_id: Snowflake) -> Place {
	let guild_id = guild_id.or_else(|| state.channel_guild(channel_id));
	let guild = guild_id.and_then(|id| state.guild(id));
	let guild_name = guild.map(|guild| guild.name.clone()).unwrap_or_default();
	let channel = state.channel(channel_id);
	Place {
		guild_id,
		acronym: acronym(&guild_name),
		guild: guild_name,
		icon: guild.and_then(|guild| guild.icon.clone()),
		channel_id,
		channel: channel
			.and_then(|channel| channel.name.clone())
			.unwrap_or_else(|| "unknown-channel".to_owned()),
		channel_icon: channel.map_or("hash", |channel| channel_icon(channel.kind)),
	}
}

pub fn mention_groups(state: &State, today: NaiveDate) -> Option<Vec<MentionGroup>> {
	let mut groups: Vec<MentionGroup> = Vec::new();
	for message in state.recent_mentions()? {
		let entry = detached_entry(state, message.guild_id, message);
		let spans = if message.content.is_empty() {
			vec![Span {
				text: ATTACHMENT_ONLY.to_owned(),
				mention: false,
			}]
		} else {
			snippet_spans(&entry.body.content, &entry.names, MENTION_TEXT)
		};
		let card = MentionCard {
			id: message.id,
			author_id: message.author.id,
			author: entry.author,
			author_color: entry.author_color,
			time: message_time(entry.sent_at, today),
			spans,
		};
		match groups.last_mut() {
			Some(group) if group.place.channel_id == message.channel_id => group.cards.push(card),
			_ => groups.push(MentionGroup {
				place: place(state, message.guild_id, message.channel_id),
				cards: vec![card],
			}),
		}
	}
	Some(groups)
}

fn rail_order(state: &State) -> Vec<Snowflake> {
	state
		.guild_layout()
		.into_iter()
		.flat_map(|entry| match entry {
			RailEntry::Guild(id) => vec![id],
			RailEntry::Folder { guilds, .. } => guilds,
		})
		.collect()
}

pub fn unread_groups(state: &State) -> Vec<UnreadGroup> {
	let unread = state.unread_channels();
	rail_order(state)
		.into_iter()
		.filter_map(|guild_id| {
			let rows: Vec<UnreadRow> = unread
				.iter()
				.filter(|channel| channel.guild_id == guild_id)
				.map(|channel| UnreadRow {
					place: place(state, Some(guild_id), channel.channel_id),
					mentions: channel.mentions,
				})
				.collect();
			if rows.is_empty() {
				return None;
			}
			let guild = state.guild(guild_id)?;
			Some(UnreadGroup {
				guild_id,
				acronym: acronym(&guild.name),
				guild: guild.name.clone(),
				icon: guild.icon.clone(),
				rows,
			})
		})
		.collect()
}

pub fn for_you(state: &State, today: NaiveDate) -> Vec<ForYouEntry> {
	let mut rows = state.pending_incoming();
	rows.sort_by(|left, right| {
		right
			.since
			.cmp(&left.since)
			.then(left.user_id.cmp(&right.user_id))
	});
	rows.into_iter()
		.map(|row| ForYouEntry {
			user_id: row.user_id,
			name: row.display,
			text: FRIEND_REQUEST,
			time: row
				.since
				.map(|since| message_time(since, today))
				.unwrap_or_default(),
		})
		.collect()
}
