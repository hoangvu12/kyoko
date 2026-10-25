use protocol::{Message, Snowflake};
use serde_json::Value;
use state::SendFailure;

#[derive(Debug, Clone, PartialEq)]
pub enum SearchStatus {
	Searching,
	Indexing,
	Ready,
	Failed(SearchFailure),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchFailure {
	StillIndexing,
	Refused(SendFailure),
}

#[derive(Debug, Clone)]
pub struct SearchResults {
	pub serial: u64,
	pub guild_id: Option<Snowflake>,
	pub channel_id: Option<Snowflake>,
	pub messages: Vec<Message>,
	pub total: u64,
	pub status: SearchStatus,
	pub(crate) exhausted: bool,
}

impl SearchResults {
	pub(crate) fn started(serial: u64, guild_id: Option<Snowflake>, channel_id: Option<Snowflake>) -> Self {
		Self {
			serial,
			guild_id,
			channel_id,
			messages: Vec::new(),
			total: 0,
			status: SearchStatus::Searching,
			exhausted: false,
		}
	}

	pub fn has_more(&self) -> bool {
		self.status == SearchStatus::Ready && !self.exhausted && (self.messages.len() as u64) < self.total
	}

	pub fn is_busy(&self) -> bool {
		matches!(self.status, SearchStatus::Searching | SearchStatus::Indexing)
	}

	pub(crate) fn add_page(&mut self, page: Page) {
		self.exhausted = page.messages.is_empty();
		for message in page.messages {
			if !self.messages.iter().any(|known| known.id == message.id) {
				self.messages.push(message);
			}
		}
		self.total = page.total.max(self.messages.len() as u64);
		self.status = SearchStatus::Ready;
	}
}

pub(crate) struct Page {
	pub(crate) messages: Vec<Message>,
	pub(crate) total: u64,
}

fn hit_of(group: &Value) -> Option<&Value> {
	let entries = group.as_array()?;
	entries
		.iter()
		.find(|entry| entry["hit"].as_bool() == Some(true))
		.or_else(|| entries.first())
}

impl Page {
	pub(crate) fn from_response(response: &Value) -> Self {
		let messages: Vec<Message> = response["messages"]
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(hit_of)
			.filter_map(|entry| serde_json::from_value(entry.clone()).ok())
			.collect();
		let total = response["total_results"]
			.as_u64()
			.unwrap_or(messages.len() as u64);
		Self { messages, total }
	}
}
