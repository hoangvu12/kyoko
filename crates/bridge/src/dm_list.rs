use std::cmp::Reverse;

use protocol::{Channel, ChannelType, Snowflake, StatusKind};
use state::{Changes, DmRow, State};

const UNNAMED_GROUP: &str = "Unnamed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmStatus {
	Online,
	Idle,
	Dnd,
	Offline,
}

impl DmStatus {
	pub fn of(state: &State, user_id: Snowflake) -> Self {
		state
			.presence(user_id)
			.map_or(Self::Offline, |presence| Self::from_kind(&presence.status))
	}

	pub fn from_kind(kind: &StatusKind) -> Self {
		match kind {
			StatusKind::Online => Self::Online,
			StatusKind::Idle => Self::Idle,
			StatusKind::Dnd => Self::Dnd,
			_ => Self::Offline,
		}
	}

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Online => "online",
			Self::Idle => "idle",
			Self::Dnd => "dnd",
			Self::Offline => "offline",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmEntry {
	pub id: Snowflake,
	pub name: String,
	pub recipient: Option<Snowflake>,
	pub status: DmStatus,
	pub members: Option<usize>,
	pub unread: bool,
	pub mentions: u32,
	pub activity: Snowflake,
}

impl DmEntry {
	pub fn is_group(&self) -> bool {
		self.members.is_some()
	}
}

#[derive(Debug, Default)]
pub struct DmList {
	entries: Vec<DmEntry>,
	requests: usize,
}

impl DmList {
	pub fn entries(&self) -> &[DmEntry] {
		&self.entries
	}

	pub fn entry(&self, channel_id: Snowflake) -> Option<&DmEntry> {
		self.entries.iter().find(|entry| entry.id == channel_id)
	}

	pub fn requests(&self) -> usize {
		self.requests
	}

	pub fn most_recent(&self) -> Option<Snowflake> {
		self.entries.first().map(|entry| entry.id)
	}

	pub fn apply(&mut self, state: &State, changes: &Changes) -> bool {
		let listing = changes.private_channels
			|| changes.read_state
			|| changes.relationships
			|| changes.user_settings
			|| changes.current_user;
		let presence = self
			.entries
			.iter()
			.filter_map(|entry| entry.recipient)
			.any(|user_id| changes.presence.contains(user_id));
		if !listing && !presence {
			return false;
		}
		self.refresh(state)
	}

	pub fn refresh(&mut self, state: &State) -> bool {
		let (entries, requests) = dm_entries(state);
		if entries == self.entries && requests == self.requests {
			return false;
		}
		self.entries = entries;
		self.requests = requests;
		true
	}
}

pub fn is_request(channel: &Channel) -> bool {
	channel.is_message_request == Some(true) || channel.is_spam == Some(true)
}

fn dm_entries(state: &State) -> (Vec<DmEntry>, usize) {
	let mut requests = 0;
	let mut entries: Vec<DmEntry> = state
		.dm_rows()
		.into_iter()
		.filter_map(|row| {
			let channel = state.channel(row.channel_id)?;
			if is_request(channel) {
				requests += 1;
				return None;
			}
			Some(dm_entry(state, channel.kind, row))
		})
		.collect();
	entries.sort_by_key(|entry| Reverse((entry.activity, entry.id)));
	(entries, requests)
}

fn dm_entry(state: &State, kind: ChannelType, row: DmRow) -> DmEntry {
	let group = kind == ChannelType::GroupDm;
	let recipient = (!group).then(|| row.recipients.first().copied()).flatten();
	let name = if group && row.name.is_empty() {
		UNNAMED_GROUP.to_owned()
	} else {
		row.name
	};
	DmEntry {
		id: row.channel_id,
		name,
		recipient,
		status: recipient.map_or(DmStatus::Offline, |user_id| DmStatus::of(state, user_id)),
		members: group.then_some(row.recipients.len() + 1),
		unread: row.unread,
		mentions: row.mentions,
		activity: row.last_message_id.unwrap_or(row.channel_id),
	}
}
