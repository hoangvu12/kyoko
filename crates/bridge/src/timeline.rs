use protocol::{Message, Snowflake};
use session::Command;
use state::{Changes, State};

use crate::sidebar::ChannelKind;

pub const WINDOW: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEntry {
	pub id: Snowflake,
	pub author: String,
	pub content: String,
	pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenChannel {
	pub guild_id: Option<Snowflake>,
	pub channel_id: Snowflake,
}

#[derive(Debug, Default)]
pub struct Timeline {
	open: Option<OpenChannel>,
	messages: Vec<MessageEntry>,
}

impl Timeline {
	pub fn open(&self) -> Option<OpenChannel> {
		self.open
	}

	pub fn messages(&self) -> &[MessageEntry] {
		&self.messages
	}

	pub fn select(
		&mut self,
		state: &State,
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	) -> Option<Command> {
		let channel = OpenChannel { guild_id, channel_id };
		if self.open == Some(channel) || state.channel(channel_id).is_none() {
			return None;
		}
		self.open = Some(channel);
		self.messages = entries(state, channel);
		Some(Command::SelectChannel { guild_id, channel_id })
	}

	pub fn close(&mut self) -> bool {
		self.open = None;
		!std::mem::take(&mut self.messages).is_empty()
	}

	pub fn apply(&mut self, state: &State, changes: &Changes) -> bool {
		let Some(open) = self.open else {
			return false;
		};
		if state.channel(open.channel_id).is_none() {
			return self.close();
		}
		if !changes.messages.contains(open.channel_id)
			&& !changes.members.contains(open.guild_id.unwrap_or_default())
		{
			return false;
		}
		let fresh = entries(state, open);
		if fresh == self.messages {
			return false;
		}
		self.messages = fresh;
		true
	}
}

fn entries(state: &State, open: OpenChannel) -> Vec<MessageEntry> {
	let Some(cache) = state.messages(open.channel_id) else {
		return Vec::new();
	};
	let skip = cache.ordered.len().saturating_sub(WINDOW);
	cache
		.ordered
		.iter()
		.skip(skip)
		.map(|message| entry(state, open.guild_id, message))
		.collect()
}

fn entry(state: &State, guild_id: Option<Snowflake>, message: &Message) -> MessageEntry {
	MessageEntry {
		id: message.id,
		author: author_name(state, guild_id, message),
		content: message.content.clone(),
		timestamp: message
			.timestamp
			.map(|at| at.format("%Y-%m-%d %H:%M").to_string())
			.unwrap_or_default(),
	}
}

fn author_name(state: &State, guild_id: Option<Snowflake>, message: &Message) -> String {
	let nick = message
		.member
		.as_ref()
		.and_then(|member| member.nick.as_deref())
		.filter(|nick| !nick.is_empty());
	if let Some(nick) = nick {
		return nick.to_owned();
	}
	let known = state.display_name(guild_id, message.author.id);
	if known != message.author.id.to_string() {
		return known;
	}
	message
		.author
		.global_name
		.as_deref()
		.filter(|name| !name.is_empty())
		.unwrap_or(&message.author.username)
		.to_owned()
}

pub fn busiest_channel(state: &State) -> Option<OpenChannel> {
	state
		.guild_summaries()
		.into_iter()
		.flat_map(|guild| {
			state
				.guild_channel_tree(guild.id)
				.into_iter()
				.flat_map(|node| node.children)
				.filter(|row| !row.locked)
				.filter(|row| {
					matches!(
						ChannelKind::of(row.kind),
						ChannelKind::Text | ChannelKind::Announcement
					)
				})
				.filter_map(|row| {
					let last = state.channel(row.id)?.last_message_id?;
					Some((last, guild.id, row.id))
				})
				.collect::<Vec<_>>()
		})
		.max_by_key(|(last, _, _)| *last)
		.map(|(_, guild_id, channel_id)| OpenChannel {
			guild_id: Some(guild_id),
			channel_id,
		})
}
