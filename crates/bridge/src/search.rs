use std::sync::Arc;

use chrono::{NaiveDate, NaiveTime};
use protocol::{ChannelType, Snowflake};
use rest::api::SearchQuery;
use session::{SearchFailure, SearchResults, SearchStatus};
use state::{SendFailure, State};

use crate::clock::message_time;
use crate::composer::failure_text;
use crate::message::snippet;
use crate::timeline::{MessageEntry, detached_entry};

pub const HIT_SNIPPET: usize = 220;
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;
const SNOWFLAKE_TIME_SHIFT: u32 = 22;
const HAS_VALUES: [&str; 8] = [
	"image", "video", "sound", "file", "sticker", "embed", "link", "poll",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFilter {
	From,
	Mentions,
	Has,
	Before,
	After,
	During,
	In,
	Pinned,
}

impl SearchFilter {
	const ALL: [Self; 8] = [
		Self::From,
		Self::Mentions,
		Self::Has,
		Self::Before,
		Self::After,
		Self::During,
		Self::In,
		Self::Pinned,
	];

	pub fn key(self) -> &'static str {
		match self {
			Self::From => "from",
			Self::Mentions => "mentions",
			Self::Has => "has",
			Self::Before => "before",
			Self::After => "after",
			Self::During => "during",
			Self::In => "in",
			Self::Pinned => "pinned",
		}
	}

	fn named(key: &str) -> Option<Self> {
		Self::ALL
			.into_iter()
			.find(|filter| filter.key().eq_ignore_ascii_case(key))
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchChip {
	pub filter: SearchFilter,
	pub value: String,
}

impl SearchChip {
	pub fn label(&self) -> String {
		format!("{}: {}", self.filter.key(), self.value)
	}
}

#[derive(Debug, Clone, Default)]
pub struct ParsedSearch {
	pub query: SearchQuery,
	pub chips: Vec<SearchChip>,
}

fn snowflake_at(date: NaiveDate, end_of_day: bool) -> Option<Snowflake> {
	let day = if end_of_day { date.succ_opt()? } else { date };
	let millis = day.and_time(NaiveTime::MIN).and_utc().timestamp_millis() - DISCORD_EPOCH_MS;
	u64::try_from(millis)
		.ok()
		.map(|millis| Snowflake(millis << SNOWFLAKE_TIME_SHIFT))
}

fn date(value: &str) -> Option<NaiveDate> {
	NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn mentioned_id(value: &str, open: &str, close: char) -> Option<u64> {
	value
		.strip_prefix(open)
		.and_then(|rest| rest.strip_suffix(close))
		.map(|digits| digits.trim_start_matches('!'))
		.unwrap_or(value)
		.parse()
		.ok()
}

fn resolve_user(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Option<Snowflake>,
	name: &str,
) -> Option<Snowflake> {
	let cleaned = name.trim_start_matches('@');
	if let Some(id) = mentioned_id(cleaned, "<@", '>') {
		return Some(Snowflake(id));
	}
	let needle = cleaned.to_lowercase();
	let candidates: Vec<Snowflake> = match guild_id.and_then(|id| state.guild(id)) {
		Some(guild) => guild.members.keys().copied().collect(),
		None => state
			.dm_rows()
			.into_iter()
			.filter(|row| Some(row.channel_id) == channel_id)
			.flat_map(|row| row.recipients)
			.chain(state.current_user_id())
			.collect(),
	};
	let labels = |id: Snowflake| {
		[
			state.display_name(guild_id, id).to_lowercase(),
			state
				.user(id)
				.map(|user| user.username.to_lowercase())
				.unwrap_or_default(),
		]
	};
	candidates
		.iter()
		.copied()
		.find(|id| labels(*id).contains(&needle))
		.or_else(|| {
			candidates
				.iter()
				.copied()
				.find(|id| labels(*id).iter().any(|label| label.starts_with(&needle)))
		})
}

fn resolve_channel(state: &State, guild_id: Option<Snowflake>, name: &str) -> Option<Snowflake> {
	let cleaned = name.trim_start_matches('#');
	if let Some(id) = mentioned_id(cleaned, "<#", '>') {
		return Some(Snowflake(id));
	}
	let guild = state.guild(guild_id?)?;
	let needle = cleaned.to_lowercase();
	guild
		.channels
		.values()
		.filter(|channel| channel.kind != ChannelType::GuildCategory)
		.find(|channel| {
			channel
				.name
				.as_deref()
				.is_some_and(|name| name.to_lowercase() == needle)
		})
		.map(|channel| channel.id)
}

pub fn parse_search(
	text: &str,
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Option<Snowflake>,
) -> ParsedSearch {
	let mut query = SearchQuery::default();
	let mut chips = Vec::new();
	let mut content: Vec<&str> = Vec::new();
	for token in text.split_whitespace() {
		let applied = token.split_once(':').and_then(|(key, value)| {
			let filter = SearchFilter::named(key)?;
			(!value.is_empty()).then_some((filter, value))
		});
		let accepted = applied.is_some_and(|(filter, value)| match filter {
			SearchFilter::From => resolve_user(state, guild_id, channel_id, value)
				.map(|id| query.author_id = Some(id))
				.is_some(),
			SearchFilter::Mentions => resolve_user(state, guild_id, channel_id, value)
				.map(|id| query.mentions = Some(id))
				.is_some(),
			SearchFilter::Has => {
				let lowered = value.to_lowercase();
				HAS_VALUES.contains(&lowered.as_str()) && {
					if !query.has.contains(&lowered) {
						query.has.push(lowered);
					}
					true
				}
			}
			SearchFilter::Before => date(value)
				.and_then(|day| snowflake_at(day, false))
				.map(|id| query.before = Some(id))
				.is_some(),
			SearchFilter::After => date(value)
				.and_then(|day| snowflake_at(day, true))
				.map(|id| query.after = Some(id))
				.is_some(),
			SearchFilter::During => date(value)
				.and_then(|day| Some((snowflake_at(day, false)?, snowflake_at(day, true)?)))
				.map(|(start, end)| {
					query.after = Some(Snowflake(start.get().saturating_sub(1)));
					query.before = Some(end);
				})
				.is_some(),
			SearchFilter::In => {
				guild_id.is_some()
					&& resolve_channel(state, guild_id, value)
						.map(|id| query.channel_id = Some(id))
						.is_some()
			}
			SearchFilter::Pinned => match value.to_lowercase().as_str() {
				"true" | "yes" => {
					query.pinned = Some(true);
					true
				}
				"false" | "no" => {
					query.pinned = Some(false);
					true
				}
				_ => false,
			},
		});
		match applied {
			Some((filter, value)) if accepted => chips.push(SearchChip {
				filter,
				value: value.to_owned(),
			}),
			_ => content.push(token),
		}
	}
	if !content.is_empty() {
		query.content = Some(content.join(" "));
	}
	ParsedSearch { query, chips }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
	pub entry: Arc<MessageEntry>,
	pub guild_id: Option<Snowflake>,
	pub channel_id: Snowflake,
	pub author: String,
	pub time: String,
	pub place: String,
	pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchPanel {
	pub serial: u64,
	pub title: String,
	pub hits: Vec<SearchHit>,
	pub notice: Option<String>,
	pub busy: bool,
	pub more: bool,
}

pub fn result_count(total: u64) -> String {
	match total {
		1 => "1 Result".to_owned(),
		count => format!("{count} Results"),
	}
}

pub fn search_notice(status: &SearchStatus) -> Option<String> {
	match status {
		SearchStatus::Searching | SearchStatus::Ready => None,
		SearchStatus::Indexing => {
			Some("Discord is still indexing these messages. Searching again in a moment.".to_owned())
		}
		SearchStatus::Failed(SearchFailure::StillIndexing) => {
			Some("Search is not ready here yet. Try again in a few minutes.".to_owned())
		}
		SearchStatus::Failed(SearchFailure::Refused(failure)) => Some(refusal_text(failure)),
	}
}

fn refusal_text(failure: &SendFailure) -> String {
	match failure {
		SendFailure::Forbidden | SendFailure::MissingPermissions => {
			"Discord refused this search. You may not be able to read these messages.".to_owned()
		}
		SendFailure::Other(Some(status)) => format!("The search failed (error {status})."),
		SendFailure::Other(None) => "The search failed.".to_owned(),
		other => failure_text(other),
	}
}

fn hit(state: &State, message: &Arc<protocol::Message>, today: NaiveDate) -> SearchHit {
	let entry = detached_entry(state, message);
	let guild_id = message
		.guild_id
		.or_else(|| state.channel_guild(message.channel_id));
	let place = state
		.channel(message.channel_id)
		.and_then(|channel| channel.name.clone())
		.filter(|_| guild_id.is_some())
		.map_or_else(String::new, |name| format!("#{name}"));
	let text = snippet(&entry.body.content, &entry.names, HIT_SNIPPET);
	SearchHit {
		guild_id,
		channel_id: message.channel_id,
		author: entry.author.clone(),
		time: message_time(entry.sent_at, today),
		place,
		text,
		entry: Arc::new(entry),
	}
}

impl SearchPanel {
	pub fn of(results: &SearchResults, state: &State, today: NaiveDate) -> Self {
		let hits = results
			.messages
			.iter()
			.map(|message| hit(state, &Arc::new(message.clone()), today))
			.collect();
		let searching = results.status == SearchStatus::Searching && results.messages.is_empty();
		Self {
			serial: results.serial,
			title: if searching {
				"Searching...".to_owned()
			} else {
				result_count(results.total)
			},
			hits,
			notice: search_notice(&results.status),
			busy: results.is_busy(),
			more: results.has_more(),
		}
	}

	pub fn hit(&self, message_id: Snowflake) -> Option<&SearchHit> {
		self.hits.iter().find(|hit| hit.entry.id == message_id)
	}
}
