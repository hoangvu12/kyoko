use protocol::Snowflake;
use state::{Changes, RelationshipRow, State};

use crate::dm_list::DmStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendsTab {
	Online,
	All,
	Pending,
	Blocked,
	Add,
}

impl FriendsTab {
	pub const ALL: [FriendsTab; 5] = [Self::Online, Self::All, Self::Pending, Self::Blocked, Self::Add];

	pub fn key(self) -> &'static str {
		match self {
			Self::Online => "online",
			Self::All => "all",
			Self::Pending => "pending",
			Self::Blocked => "blocked",
			Self::Add => "add",
		}
	}

	pub fn named(key: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|tab| tab.key() == key)
	}

	pub fn empty_text(self) -> &'static str {
		match self {
			Self::Online => "No friends are online right now.",
			Self::All => "You have no friends yet.",
			Self::Pending => "There are no pending friend requests.",
			Self::Blocked | Self::Add => "You have not blocked anyone.",
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendKind {
	Friend,
	Incoming,
	Outgoing,
	Blocked,
}

impl FriendKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Friend => "friend",
			Self::Incoming => "incoming",
			Self::Outgoing => "outgoing",
			Self::Blocked => "blocked",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendEntry {
	pub id: Snowflake,
	pub name: String,
	pub username: String,
	pub avatar: Option<String>,
	pub status: DmStatus,
	pub activity: Option<String>,
	pub kind: FriendKind,
}

impl FriendEntry {
	fn of(row: RelationshipRow, kind: FriendKind) -> Self {
		Self {
			id: row.user_id,
			name: row.display,
			username: row.username,
			avatar: row.avatar,
			status: DmStatus::from_kind(&row.status),
			activity: row.activity,
			kind,
		}
	}

	pub fn subtitle(&self) -> &str {
		match self.kind {
			FriendKind::Incoming => "Incoming Friend Request",
			FriendKind::Outgoing => "Outgoing Friend Request",
			FriendKind::Blocked => "Blocked",
			FriendKind::Friend => self
				.activity
				.as_deref()
				.unwrap_or_else(|| status_label(self.status)),
		}
	}

	fn matches(&self, filter: &str) -> bool {
		filter.is_empty()
			|| self.name.to_lowercase().contains(filter)
			|| self.username.to_lowercase().contains(filter)
	}
}

pub fn status_label(status: DmStatus) -> &'static str {
	match status {
		DmStatus::Online => "Online",
		DmStatus::Idle => "Idle",
		DmStatus::Dnd => "Do Not Disturb",
		DmStatus::Offline => "Offline",
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendSection {
	pub title: String,
	pub entries: Vec<FriendEntry>,
}

#[derive(Debug, Default)]
pub struct FriendsView {
	friends: Vec<FriendEntry>,
	incoming: Vec<FriendEntry>,
	outgoing: Vec<FriendEntry>,
	blocked: Vec<FriendEntry>,
}

impl FriendsView {
	pub fn of(state: &State) -> Self {
		let entries = |rows: Vec<RelationshipRow>, kind| {
			rows.into_iter().map(|row| FriendEntry::of(row, kind)).collect()
		};
		Self {
			friends: entries(state.friends(), FriendKind::Friend),
			incoming: entries(state.pending_incoming(), FriendKind::Incoming),
			outgoing: entries(state.pending_outgoing(), FriendKind::Outgoing),
			blocked: entries(state.blocked(), FriendKind::Blocked),
		}
	}

	pub fn apply(&mut self, state: &State, changes: &Changes) -> bool {
		let presence = self.everyone().any(|entry| changes.presence.contains(entry.id));
		if !changes.relationships && !presence {
			return false;
		}
		self.refresh(state)
	}

	pub fn refresh(&mut self, state: &State) -> bool {
		let next = Self::of(state);
		let same = next.friends == self.friends
			&& next.incoming == self.incoming
			&& next.outgoing == self.outgoing
			&& next.blocked == self.blocked;
		if !same {
			*self = next;
		}
		!same
	}

	pub fn everyone(&self) -> impl Iterator<Item = &FriendEntry> {
		self.friends
			.iter()
			.chain(&self.incoming)
			.chain(&self.outgoing)
			.chain(&self.blocked)
	}

	pub fn pending_count(&self) -> usize {
		self.incoming.len()
	}

	pub fn sections(&self, tab: FriendsTab, filter: &str) -> Vec<FriendSection> {
		let filter = filter.trim().to_lowercase();
		let pick = |title: &str, entries: &[FriendEntry], online_only: bool| {
			let entries: Vec<FriendEntry> = entries
				.iter()
				.filter(|entry| !online_only || entry.status != DmStatus::Offline)
				.filter(|entry| entry.matches(&filter))
				.cloned()
				.collect();
			FriendSection {
				title: format!("{title} — {}", entries.len()).to_uppercase(),
				entries,
			}
		};
		let sections = match tab {
			FriendsTab::Online => vec![pick("Online", &self.friends, true)],
			FriendsTab::All => vec![pick("All Friends", &self.friends, false)],
			FriendsTab::Pending => vec![
				pick("Incoming", &self.incoming, false),
				pick("Outgoing", &self.outgoing, false),
			],
			FriendsTab::Blocked => vec![pick("Blocked", &self.blocked, false)],
			FriendsTab::Add => Vec::new(),
		};
		sections
			.into_iter()
			.filter(|section| !section.entries.is_empty())
			.collect()
	}

	pub fn active_now(&self) -> Vec<&FriendEntry> {
		self.friends
			.iter()
			.filter(|entry| entry.activity.is_some())
			.collect()
	}

	pub fn entry(&self, user_id: Snowflake) -> Option<&FriendEntry> {
		self.everyone().find(|entry| entry.id == user_id)
	}
}
