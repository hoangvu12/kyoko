use protocol::{ChannelType, Snowflake};
use session::Command;
use state::{Changes, ChannelRow, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
	Category,
	Text,
	Announcement,
	Voice,
	Stage,
	Forum,
	Thread,
	Other,
}

impl ChannelKind {
	pub fn of(kind: ChannelType) -> Self {
		match kind {
			ChannelType::GuildCategory => Self::Category,
			ChannelType::GuildText => Self::Text,
			ChannelType::GuildAnnouncement => Self::Announcement,
			ChannelType::GuildVoice => Self::Voice,
			ChannelType::GuildStageVoice => Self::Stage,
			ChannelType::GuildForum | ChannelType::GuildMedia => Self::Forum,
			ChannelType::AnnouncementThread | ChannelType::PublicThread | ChannelType::PrivateThread => {
				Self::Thread
			}
			_ => Self::Other,
		}
	}

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Category => "category",
			Self::Text => "text",
			Self::Announcement => "announcement",
			Self::Voice => "voice",
			Self::Stage => "stage",
			Self::Forum => "forum",
			Self::Thread => "thread",
			Self::Other => "other",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildEntry {
	pub id: Snowflake,
	pub name: String,
	pub unread: bool,
	pub mentions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEntry {
	pub id: Snowflake,
	pub name: String,
	pub kind: ChannelKind,
	pub unread: bool,
	pub mentions: u32,
	pub muted: bool,
}

impl ChannelEntry {
	fn from_row(row: &ChannelRow) -> Self {
		Self {
			id: row.id,
			name: row.name.clone(),
			kind: ChannelKind::of(row.kind),
			unread: row.unread,
			mentions: row.mentions,
			muted: row.muted,
		}
	}
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Refresh {
	pub guilds: bool,
	pub channels: bool,
	pub selected: bool,
}

#[derive(Debug, Default)]
pub struct Sidebar {
	selected: Option<Snowflake>,
	guilds: Vec<GuildEntry>,
	channels: Vec<ChannelEntry>,
}

impl Sidebar {
	pub fn guilds(&self) -> &[GuildEntry] {
		&self.guilds
	}

	pub fn channels(&self) -> &[ChannelEntry] {
		&self.channels
	}

	pub fn selected(&self) -> Option<Snowflake> {
		self.selected
	}

	pub fn apply(&mut self, state: &State, changes: &Changes) -> Refresh {
		let mut refresh = Refresh::default();
		let listing = changes.guild_list || changes.read_state || changes.user_settings;
		if listing {
			refresh.guilds = replace(&mut self.guilds, guild_entries(state));
		}
		let selected = self
			.selected
			.filter(|id| state.guild(*id).is_some())
			.or_else(|| self.guilds.first().map(|guild| guild.id));
		if selected != self.selected {
			self.selected = selected;
			refresh.selected = true;
		}
		let touched = self.selected.is_some_and(|id| changes.channel_lists.contains(id));
		if listing || refresh.selected || touched {
			refresh.channels = self.refresh_channels(state);
		}
		refresh
	}

	pub fn select(&mut self, state: &State, guild_id: Snowflake) -> Option<Command> {
		if state.guild(guild_id).is_none() || self.selected == Some(guild_id) {
			return None;
		}
		self.selected = Some(guild_id);
		self.refresh_channels(state);
		Some(Command::SubscribeGuild { guild_id })
	}

	fn refresh_channels(&mut self, state: &State) -> bool {
		let channels = self
			.selected
			.map(|id| channel_entries(state, id))
			.unwrap_or_default();
		replace(&mut self.channels, channels)
	}
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
	if *slot == value {
		return false;
	}
	*slot = value;
	true
}

fn guild_entries(state: &State) -> Vec<GuildEntry> {
	state
		.guild_summaries()
		.into_iter()
		.map(|summary| GuildEntry {
			id: summary.id,
			name: summary.name,
			unread: summary.unread,
			mentions: summary.mentions,
		})
		.collect()
}

fn channel_entries(state: &State, guild_id: Snowflake) -> Vec<ChannelEntry> {
	let mut entries = Vec::new();
	for node in state.guild_channel_tree(guild_id) {
		let visible: Vec<&ChannelRow> = node.children.iter().filter(|row| shown(row)).collect();
		match &node.category {
			Some(_) if visible.is_empty() => continue,
			Some(category) => entries.push(ChannelEntry::from_row(category)),
			None => {}
		}
		entries.extend(visible.into_iter().map(ChannelEntry::from_row));
	}
	entries
}

fn shown(row: &ChannelRow) -> bool {
	if row.locked {
		return false;
	}
	ChannelKind::of(row.kind) != ChannelKind::Thread || row.member
}
