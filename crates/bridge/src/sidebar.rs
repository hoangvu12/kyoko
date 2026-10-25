use protocol::{ChannelType, Permissions, Snowflake, VoiceState};
use session::Command;
use state::{Changes, ChannelRow, ChannelTreeNode, State};

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

	pub fn opens_timeline(self) -> bool {
		matches!(
			self,
			Self::Text | Self::Announcement | Self::Voice | Self::Stage | Self::Thread
		)
	}
}

pub fn channel_icon(kind: ChannelType) -> &'static str {
	match kind {
		ChannelType::GuildVoice => "volume",
		ChannelType::GuildStageVoice => "stage",
		ChannelType::GuildAnnouncement | ChannelType::AnnouncementThread => "megaphone",
		ChannelType::GuildForum | ChannelType::GuildMedia => "forum",
		ChannelType::PublicThread | ChannelType::PrivateThread => "thread",
		ChannelType::Dm | ChannelType::GroupDm => "at_sign",
		_ => "hash",
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildEntry {
	pub id: Snowflake,
	pub name: String,
	pub unread: bool,
	pub mentions: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceVideo {
	Camera,
	Screen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceMember {
	pub user_id: Snowflake,
	pub name: String,
	pub muted: bool,
	pub deafened: bool,
	pub video: Option<VoiceVideo>,
	pub live: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEntry {
	pub id: Snowflake,
	pub name: String,
	pub kind: ChannelKind,
	pub icon: &'static str,
	pub unread: bool,
	pub mentions: u32,
	pub muted: bool,
	pub locked: bool,
	pub joined: bool,
	pub thread: bool,
	pub last_thread: bool,
	pub voice: Vec<VoiceMember>,
}

impl ChannelEntry {
	pub fn selectable(&self) -> bool {
		!self.locked && self.kind != ChannelKind::Category
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryEntry {
	pub id: Snowflake,
	pub name: String,
	pub collapsed: bool,
	pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSection {
	pub category: Option<CategoryEntry>,
	pub channels: Vec<ChannelEntry>,
}

impl SidebarSection {
	fn collapsed(&self) -> bool {
		self.category.as_ref().is_some_and(|category| category.collapsed)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarRow {
	Category(CategoryEntry),
	Channel(ChannelEntry),
}

impl SidebarRow {
	pub fn id(&self) -> Snowflake {
		match self {
			Self::Category(category) => category.id,
			Self::Channel(channel) => channel.id,
		}
	}
}

const SETTINGS_PERMISSIONS: Permissions = Permissions::MANAGE_GUILD
	.union(Permissions::MANAGE_ROLES)
	.union(Permissions::MANAGE_GUILD_EXPRESSIONS)
	.union(Permissions::VIEW_AUDIT_LOG)
	.union(Permissions::BAN_MEMBERS)
	.union(Permissions::KICK_MEMBERS)
	.union(Permissions::MODERATE_MEMBERS);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildHeader {
	pub name: String,
	pub unread: bool,
	pub muted: bool,
	pub owner: bool,
	pub can_invite: bool,
	pub can_manage_channels: bool,
	pub can_open_settings: bool,
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
	open: Option<Snowflake>,
	guilds: Vec<GuildEntry>,
	header: GuildHeader,
	sections: Vec<SidebarSection>,
	rows: Vec<SidebarRow>,
}

impl Sidebar {
	pub fn guilds(&self) -> &[GuildEntry] {
		&self.guilds
	}

	pub fn channels(&self) -> impl Iterator<Item = &ChannelEntry> {
		self.sections.iter().flat_map(|section| section.channels.iter())
	}

	pub fn channel(&self, id: Snowflake) -> Option<&ChannelEntry> {
		self.channels().find(|channel| channel.id == id)
	}

	pub fn sections(&self) -> &[SidebarSection] {
		&self.sections
	}

	pub fn rows(&self) -> &[SidebarRow] {
		&self.rows
	}

	pub fn header(&self) -> &GuildHeader {
		&self.header
	}

	pub fn selected(&self) -> Option<Snowflake> {
		self.selected
	}

	pub fn open_channel(&self) -> Option<Snowflake> {
		self.open
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
		self.open = None;
		self.refresh_channels(state);
		Some(Command::SubscribeGuild { guild_id })
	}

	pub fn set_open(&mut self, channel_id: Option<Snowflake>) -> bool {
		if self.open == channel_id {
			return false;
		}
		self.open = channel_id;
		replace(&mut self.rows, visible_rows(&self.sections, self.open))
	}

	pub fn toggle_command(&self, category_id: Snowflake) -> Option<Command> {
		let guild_id = self.selected?;
		self.sections
			.iter()
			.filter_map(|section| section.category.as_ref())
			.any(|category| category.id == category_id)
			.then_some(Command::ToggleCategory {
				guild_id,
				category_id,
			})
	}

	fn refresh_channels(&mut self, state: &State) -> bool {
		let (header, sections) = match self.selected {
			Some(id) => (guild_header(state, id), sections(state, id)),
			None => (GuildHeader::default(), Vec::new()),
		};
		let header_changed = replace(&mut self.header, header);
		let sections_changed = replace(&mut self.sections, sections);
		let rows_changed = replace(&mut self.rows, visible_rows(&self.sections, self.open));
		header_changed || sections_changed || rows_changed
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

fn guild_header(state: &State, guild_id: Snowflake) -> GuildHeader {
	let summary = state.guild_summary(guild_id);
	let guild = state.guild(guild_id);
	let me = state.current_user_id();
	let owner = guild.is_some_and(|guild| guild.owner_id.is_some() && guild.owner_id == me);
	let permissions = me.map_or(Permissions::empty(), |me| state.guild_permissions(guild_id, me));
	let admin = owner || permissions.contains(Permissions::ADMINISTRATOR);
	GuildHeader {
		name: guild.map(|guild| guild.name.clone()).unwrap_or_default(),
		owner,
		can_invite: admin || permissions.contains(Permissions::CREATE_INSTANT_INVITE),
		can_manage_channels: admin || permissions.contains(Permissions::MANAGE_CHANNELS),
		can_open_settings: admin || permissions.intersects(SETTINGS_PERMISSIONS),
		unread: summary
			.as_ref()
			.is_some_and(|summary| summary.unread || summary.mentions > 0),
		muted: summary.is_some_and(|summary| summary.muted),
	}
}

fn sections(state: &State, guild_id: Snowflake) -> Vec<SidebarSection> {
	state
		.guild_channel_tree(guild_id)
		.iter()
		.map(|node| section(state, guild_id, node))
		.collect()
}

fn section(state: &State, guild_id: Snowflake, node: &ChannelTreeNode) -> SidebarSection {
	let mut channels: Vec<ChannelEntry> = node
		.children
		.iter()
		.map(|row| channel_entry(state, guild_id, row))
		.collect();
	let ends: Vec<bool> = channels
		.iter()
		.enumerate()
		.map(|(index, channel)| channel.thread && !channels.get(index + 1).is_some_and(|next| next.thread))
		.collect();
	for (channel, last) in channels.iter_mut().zip(ends) {
		channel.last_thread = last;
	}
	SidebarSection {
		category: node.category.as_ref().map(|category| CategoryEntry {
			id: category.id,
			name: category.name.clone(),
			collapsed: node.collapsed,
			muted: category.muted,
		}),
		channels,
	}
}

fn channel_entry(state: &State, guild_id: Snowflake, row: &ChannelRow) -> ChannelEntry {
	let kind = ChannelKind::of(row.kind);
	ChannelEntry {
		id: row.id,
		name: row.name.clone(),
		kind,
		icon: channel_icon(row.kind),
		unread: row.unread,
		mentions: row.mentions,
		muted: row.muted,
		locked: row.locked,
		joined: row.member,
		thread: kind == ChannelKind::Thread,
		last_thread: false,
		voice: if matches!(kind, ChannelKind::Voice | ChannelKind::Stage) {
			voice_members(state, guild_id, row.id)
		} else {
			Vec::new()
		},
	}
}

fn voice_members(state: &State, guild_id: Snowflake, channel_id: Snowflake) -> Vec<VoiceMember> {
	let Some(guild) = state.guild(guild_id) else {
		return Vec::new();
	};
	let mut members: Vec<VoiceMember> = guild
		.voice_states
		.values()
		.filter(|voice| voice.channel_id == Some(channel_id))
		.map(|voice| voice_member(state, guild_id, voice))
		.collect();
	members.sort_by_cached_key(|member| (member.name.to_lowercase(), member.user_id));
	members
}

fn voice_member(state: &State, guild_id: Snowflake, voice: &VoiceState) -> VoiceMember {
	let streaming = voice.self_stream == Some(true);
	VoiceMember {
		user_id: voice.user_id,
		name: voice_name(state, guild_id, voice),
		muted: voice.mute || voice.self_mute,
		deafened: voice.deaf || voice.self_deaf,
		video: if voice.self_video {
			Some(VoiceVideo::Camera)
		} else if streaming {
			Some(VoiceVideo::Screen)
		} else {
			None
		},
		live: streaming,
	}
}

fn voice_name(state: &State, guild_id: Snowflake, voice: &VoiceState) -> String {
	let listed = state
		.guild(guild_id)
		.is_some_and(|guild| guild.members.get(&voice.user_id).is_some());
	let nick = voice
		.member
		.as_ref()
		.and_then(|member| member.nick.as_deref())
		.filter(|nick| !nick.is_empty());
	match nick {
		Some(nick) if !listed => nick.to_owned(),
		_ => state.display_name(Some(guild_id), voice.user_id),
	}
}

fn visible_rows(sections: &[SidebarSection], open: Option<Snowflake>) -> Vec<SidebarRow> {
	let mut rows = Vec::new();
	for section in sections {
		if let Some(category) = &section.category {
			rows.push(SidebarRow::Category(category.clone()));
		}
		let collapsed = section.collapsed();
		rows.extend(
			section
				.channels
				.iter()
				.filter(|channel| {
					!collapsed || Some(channel.id) == open || channel.unread || !channel.voice.is_empty()
				})
				.cloned()
				.map(SidebarRow::Channel),
		);
	}
	rows
}
