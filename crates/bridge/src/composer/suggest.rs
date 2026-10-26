use std::collections::HashMap;
use std::time::Instant;

use protocol::expressions::unicode::search_unicode;
use protocol::{ApplicationCommand, ApplicationCommandIndex, ApplicationCommandType, ChannelType, Snowflake};
use state::{EmojiCandidate, MemberListRow, State};

use crate::emoji_picker::emoji_lock;

const MAX_SUGGESTIONS: usize = 8;
const MAX_EMOJI_SUGGESTIONS: usize = 9;
const LOCKED_ROWS: usize = 2;
const MIN_EMOJI_QUERY: usize = 2;
const RECENT_AUTHORS: usize = 200;
const RANK_CANDIDATES: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
	Mention,
	Channel,
	Emoji,
	Command,
}

impl Trigger {
	fn of(character: char) -> Option<Trigger> {
		match character {
			'@' => Some(Trigger::Mention),
			'#' => Some(Trigger::Channel),
			':' => Some(Trigger::Emoji),
			'/' => Some(Trigger::Command),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
	pub trigger: Trigger,
	pub start: usize,
	pub end: usize,
	pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visual {
	Avatar(Snowflake),
	Glyph(String),
	Emoji { id: Snowflake, animated: bool },
	Icon(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
	pub label: String,
	pub detail: Option<String>,
	pub insert: String,
	pub visual: Visual,
	pub lock: Option<&'static str>,
}

pub fn query_at(text: &str, caret: usize) -> Option<Query> {
	let mut caret = caret.min(text.len());
	while !text.is_char_boundary(caret) {
		caret -= 1;
	}
	let before = &text[..caret];
	let start = before.rfind(char::is_whitespace).map_or(0, |index| {
		index + before[index..].chars().next().map_or(1, char::len_utf8)
	});
	let token = &before[start..];
	let first = token.chars().next()?;
	let trigger = Trigger::of(first)?;
	let query = &token[first.len_utf8()..];
	let usable = match trigger {
		Trigger::Emoji => query.chars().count() >= MIN_EMOJI_QUERY && !query.contains(':'),
		Trigger::Command => {
			start == 0 && !text[caret..].contains(char::is_whitespace) && !before.contains('\n')
		}
		Trigger::Mention | Trigger::Channel => true,
	};
	usable.then(|| Query {
		trigger,
		start,
		end: caret,
		text: query.to_lowercase(),
	})
}

fn rank(name: &str, query: &str) -> Option<u8> {
	let name = name.to_lowercase();
	if name.starts_with(query) {
		Some(0)
	} else {
		name.contains(query).then_some(1)
	}
}

fn channel_icon(kind: ChannelType) -> &'static str {
	match kind {
		ChannelType::GuildVoice => "volume",
		ChannelType::GuildStageVoice => "stage",
		ChannelType::GuildAnnouncement => "megaphone",
		ChannelType::GuildForum | ChannelType::GuildMedia => "forum",
		ChannelType::AnnouncementThread | ChannelType::PublicThread | ChannelType::PrivateThread => "thread",
		_ => "hash",
	}
}

fn mention_candidates(state: &State, guild_id: Option<Snowflake>, channel_id: Snowflake) -> Vec<Snowflake> {
	let mut candidates = Vec::new();
	if let Some(guild_id) = guild_id {
		for row in state.member_rows(guild_id) {
			if let MemberListRow::Member(member) = row {
				candidates.push(member.user_id);
			}
		}
		if let Some(guild) = state.guild(guild_id) {
			candidates.extend(guild.members.keys().copied());
		}
	}
	if let Some(cache) = state.messages(channel_id) {
		candidates.extend(
			cache
				.ordered
				.iter()
				.rev()
				.take(RECENT_AUTHORS)
				.map(|message| message.author.id),
		);
	}
	if let Some(channel) = state.channel(channel_id) {
		candidates.extend(channel.recipient_ids.iter().copied());
	}
	let me = state.current_user_id();
	let mut seen = Vec::with_capacity(candidates.len());
	for id in candidates {
		if Some(id) != me && !seen.contains(&id) {
			seen.push(id);
		}
	}
	seen
}

fn mentions(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	query: &str,
) -> Vec<(u8, Suggestion)> {
	let mut found = Vec::new();
	for id in mention_candidates(state, guild_id, channel_id) {
		let display = state.display_name(guild_id, id);
		let username = state.user(id).map(|user| user.username.clone());
		let hit = rank(&display, query).or_else(|| username.as_deref().and_then(|name| rank(name, query)));
		if let Some(hit) = hit {
			found.push((
				hit,
				Suggestion {
					label: display,
					detail: username,
					insert: format!("<@{id}> "),
					visual: Visual::Avatar(id),
					lock: None,
				},
			));
		}
	}
	if let Some(guild) = guild_id.and_then(|id| state.guild(id)) {
		for role in guild.roles.values() {
			if role.id == guild.id || !role.mentionable {
				continue;
			}
			if let Some(hit) = rank(&role.name, query) {
				found.push((
					hit + 2,
					Suggestion {
						label: format!("@{}", role.name),
						detail: None,
						insert: format!("<@&{}> ", role.id),
						visual: Visual::Icon("at_sign"),
						lock: None,
					},
				));
			}
		}
	}
	for everyone in ["everyone", "here"] {
		if guild_id.is_some()
			&& let Some(hit) = rank(everyone, query)
		{
			found.push((
				hit + 2,
				Suggestion {
					label: format!("@{everyone}"),
					detail: None,
					insert: format!("@{everyone} "),
					visual: Visual::Icon("at_sign"),
					lock: None,
				},
			));
		}
	}
	found
}

fn channels(state: &State, guild_id: Option<Snowflake>, query: &str) -> Vec<(u8, Suggestion)> {
	let Some(guild_id) = guild_id else {
		return Vec::new();
	};
	let mut found = Vec::new();
	for node in state.guild_channel_tree(guild_id) {
		let category = node.category.as_ref().map(|category| category.name.clone());
		for channel in node.children.iter().filter(|channel| !channel.locked) {
			if let Some(hit) = rank(&channel.name, query) {
				found.push((
					hit,
					Suggestion {
						label: channel.name.clone(),
						detail: category.clone(),
						insert: format!("<#{}> ", channel.id),
						visual: Visual::Icon(channel_icon(channel.kind)),
						lock: None,
					},
				));
			}
		}
	}
	found
}

struct EmojiMatch {
	suggestion: Suggestion,
	candidate: EmojiCandidate,
}

fn emoji_matches(state: &State, guild_id: Option<Snowflake>, query: &str) -> Vec<EmojiMatch> {
	let nitro = state.has_nitro();
	let mut found = Vec::new();
	for section in state.emoji_catalog(guild_id).guilds {
		for emoji in &section.emojis {
			if let Some(hit) = rank(&emoji.name, query) {
				let prefix = if emoji.animated { "a" } else { "" };
				found.push((
					hit,
					EmojiMatch {
						suggestion: Suggestion {
							label: format!(":{}:", emoji.name),
							detail: Some(section.name.clone()),
							insert: format!("<{prefix}:{}:{}> ", emoji.name, emoji.id),
							visual: Visual::Emoji {
								id: emoji.id,
								animated: emoji.animated,
							},
							lock: emoji_lock(emoji, nitro),
						},
						candidate: EmojiCandidate {
							key: emoji.id.to_string(),
							name: emoji.name.clone(),
							id: Some(emoji.id),
							animated: emoji.animated,
							guild_id: Some(section.guild_id),
							unicode: None,
						},
					},
				));
			}
		}
	}
	for (order, emoji) in search_unicode(query).into_iter().enumerate() {
		found.push((
			u8::from(order > 0 && !emoji.shortcode.starts_with(query)),
			EmojiMatch {
				suggestion: Suggestion {
					label: format!(":{}:", emoji.shortcode),
					detail: None,
					insert: format!("{} ", emoji.emoji),
					visual: Visual::Glyph(emoji.emoji.to_owned()),
					lock: None,
				},
				candidate: EmojiCandidate {
					key: emoji.shortcode.to_owned(),
					name: emoji.shortcode.to_owned(),
					id: None,
					animated: false,
					guild_id: None,
					unicode: Some(emoji.emoji.to_owned()),
				},
			},
		));
	}
	found.sort_by_key(|(hit, _)| *hit);
	found.into_iter().map(|(_, found)| found).collect()
}

fn ranked(matches: Vec<EmojiMatch>, order: Option<&[String]>) -> Vec<Suggestion> {
	let order = order.unwrap_or_default();
	let positions: HashMap<&str, usize> = matches
		.iter()
		.enumerate()
		.map(|(index, found)| (found.candidate.key.as_str(), index))
		.collect();
	let first: Vec<usize> = order
		.iter()
		.filter_map(|key| positions.get(key.as_str()).copied())
		.collect();
	let mut slots: Vec<Option<Suggestion>> =
		matches.into_iter().map(|found| Some(found.suggestion)).collect();
	let mut ranked: Vec<Suggestion> = first
		.into_iter()
		.filter_map(|index| slots[index].take())
		.collect();
	ranked.extend(slots.into_iter().flatten());
	with_locked_last(ranked)
}

fn with_locked_last(ranked: Vec<Suggestion>) -> Vec<Suggestion> {
	let (mut shown, locked): (Vec<Suggestion>, Vec<Suggestion>) = ranked
		.into_iter()
		.partition(|suggestion| suggestion.lock.is_none());
	shown.truncate(MAX_EMOJI_SUGGESTIONS - locked.len().min(LOCKED_ROWS));
	let room = MAX_EMOJI_SUGGESTIONS - shown.len();
	shown.extend(locked.into_iter().take(room));
	shown
}

pub fn suggestion_heading(query: &Query) -> (&'static str, String) {
	let (title, sign) = match query.trigger {
		Trigger::Emoji => ("Emoji matching", ':'),
		Trigger::Mention => ("Members matching", '@'),
		Trigger::Channel => ("Channels matching", '#'),
		Trigger::Command => ("Commands matching", '/'),
	};
	(title, format!("{sign}{}", query.text))
}

pub fn emoji_candidates(state: &State, guild_id: Option<Snowflake>, query: &Query) -> Vec<EmojiCandidate> {
	if query.trigger != Trigger::Emoji {
		return Vec::new();
	}
	emoji_matches(state, guild_id, &query.text)
		.into_iter()
		.filter(|found| found.suggestion.lock.is_none())
		.enumerate()
		.filter(|(index, found)| {
			*index < RANK_CANDIDATES
				|| state
					.favorite_position(found.candidate.id, found.candidate.unicode.as_deref())
					.is_some()
		})
		.map(|(_, found)| found.candidate)
		.collect()
}

pub(super) fn chat_commands(index: &ApplicationCommandIndex) -> impl Iterator<Item = &ApplicationCommand> {
	index
		.application_commands
		.iter()
		.filter(|command| command.kind == ApplicationCommandType::ChatInput)
}

fn commands(index: Option<&ApplicationCommandIndex>, query: &str) -> Vec<(u8, Suggestion)> {
	let Some(index) = index else {
		return Vec::new();
	};
	chat_commands(index)
		.filter_map(|command| {
			let hit = rank(&command.name, query)?;
			let application = index
				.application(command.application_id)
				.map(|application| application.name.clone());
			let detail = match (command.description.as_deref(), application) {
				(Some(description), Some(application)) if !description.is_empty() => {
					format!("{description} · {application}")
				}
				(Some(description), None) => description.to_owned(),
				(_, Some(application)) => application,
				(None, None) => String::new(),
			};
			Some((
				hit,
				Suggestion {
					label: format!("/{}", command.name),
					detail: (!detail.is_empty()).then_some(detail),
					insert: format!("/{} ", command.name),
					visual: Visual::Icon("plugins"),
					lock: None,
				},
			))
		})
		.collect()
}

pub fn suggestions(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	query: &Query,
) -> Vec<Suggestion> {
	let mut found = match query.trigger {
		Trigger::Mention => mentions(state, guild_id, channel_id, &query.text),
		Trigger::Channel => channels(state, guild_id, &query.text),
		Trigger::Emoji => {
			let matches = emoji_matches(state, guild_id, &query.text);
			return ranked(matches, state.emoji_ranking(guild_id, &query.text));
		}
		Trigger::Command => commands(state.command_index(channel_id).map(AsRef::as_ref), &query.text),
	};
	found.sort_by_key(|(hit, _)| *hit);
	found
		.into_iter()
		.take(MAX_SUGGESTIONS)
		.map(|(_, suggestion)| suggestion)
		.collect()
}

pub fn apply_suggestion(text: &str, query: &Query, suggestion: &Suggestion) -> (String, usize) {
	let start = query.start.min(text.len());
	let end = query.end.clamp(start, text.len());
	let mut applied = String::with_capacity(text.len() + suggestion.insert.len());
	applied.push_str(&text[..start]);
	applied.push_str(&suggestion.insert);
	let caret = applied.len();
	applied.push_str(&text[end..]);
	(applied, caret)
}

pub fn typing_line(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	now: Instant,
) -> String {
	let names: Vec<String> = state
		.typing_users(channel_id, now)
		.into_iter()
		.map(|user_id| state.display_name(guild_id, user_id))
		.collect();
	match names.as_slice() {
		[] => String::new(),
		[one] => format!("{one} is typing..."),
		[first, second] => format!("{first} and {second} are typing..."),
		_ => "Several people are typing...".to_owned(),
	}
}

pub fn placeholder(state: &State, channel_id: Snowflake) -> String {
	let Some(channel) = state.channel(channel_id) else {
		return String::new();
	};
	match channel.kind {
		ChannelType::Dm => {
			let name = channel
				.recipient_ids
				.first()
				.map(|user_id| state.display_name(None, *user_id))
				.unwrap_or_default();
			format!("Message @{name}")
		}
		ChannelType::GroupDm => format!("Message {}", channel.name.clone().unwrap_or_default()),
		_ => format!("Message #{}", channel.name.clone().unwrap_or_default()),
	}
}
