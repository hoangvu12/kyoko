use std::cmp::Reverse;

use chrono::{DateTime, NaiveDate, Utc};
use protocol::{ChannelType, Permissions, Role, Snowflake};
use serde_json::Value;
use state::{AuditEntry, State};

use crate::clock::message_time;
use crate::profile::RoleChip;

pub const MEMBER_ROW_LIMIT: usize = 150;
const VALUE_LIMIT: usize = 60;
const CHANGE_LIMIT: usize = 5;

pub const MODERATION_PERMISSIONS: Permissions = Permissions::MANAGE_NICKNAMES
	.union(Permissions::MANAGE_ROLES)
	.union(Permissions::MOVE_MEMBERS)
	.union(Permissions::MUTE_MEMBERS)
	.union(Permissions::DEAFEN_MEMBERS)
	.union(Permissions::MODERATE_MEMBERS)
	.union(Permissions::KICK_MEMBERS)
	.union(Permissions::BAN_MEMBERS);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuildAccess {
	pub permissions: Permissions,
	pub owner: bool,
	pub top_role: i64,
}

impl GuildAccess {
	pub fn of(state: &State, guild_id: Snowflake) -> Self {
		let Some(me) = state.current_user_id() else {
			return Self::default();
		};
		let Some(guild) = state.guild(guild_id) else {
			return Self::default();
		};
		Self {
			permissions: state.guild_permissions(guild_id, me),
			owner: guild.owner_id == Some(me),
			top_role: state.top_role_position(guild_id, me),
		}
	}

	pub fn has(&self, permission: Permissions) -> bool {
		self.permissions.contains(Permissions::ADMINISTRATOR) || self.permissions.contains(permission)
	}

	pub fn has_any(&self, permissions: Permissions) -> bool {
		self.permissions.contains(Permissions::ADMINISTRATOR) || self.permissions.intersects(permissions)
	}

	pub fn above(&self, position: i64) -> bool {
		self.owner || position < self.top_role
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerPage {
	Overview,
	Roles,
	Emoji,
	Stickers,
	Members,
	Invites,
	Bans,
	AuditLog,
}

pub const SERVER_GROUPS: [(&str, &[ServerPage]); 4] = [
	("", &[ServerPage::Overview]),
	("Expression", &[ServerPage::Emoji, ServerPage::Stickers]),
	(
		"People",
		&[ServerPage::Members, ServerPage::Roles, ServerPage::Invites],
	),
	("Moderation", &[ServerPage::AuditLog, ServerPage::Bans]),
];

impl ServerPage {
	pub const ALL: [ServerPage; 8] = [
		Self::Overview,
		Self::Roles,
		Self::Emoji,
		Self::Stickers,
		Self::Members,
		Self::Invites,
		Self::Bans,
		Self::AuditLog,
	];

	pub fn key(self) -> &'static str {
		match self {
			Self::Overview => "overview",
			Self::Roles => "roles",
			Self::Emoji => "emoji",
			Self::Stickers => "stickers",
			Self::Members => "members",
			Self::Invites => "invites",
			Self::Bans => "bans",
			Self::AuditLog => "audit-log",
		}
	}

	pub fn named(key: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|page| page.key() == key)
	}

	pub fn title(self) -> &'static str {
		match self {
			Self::Overview => "Overview",
			Self::Roles => "Roles",
			Self::Emoji => "Emoji",
			Self::Stickers => "Stickers",
			Self::Members => "Members",
			Self::Invites => "Invites",
			Self::Bans => "Bans",
			Self::AuditLog => "Audit Log",
		}
	}

	pub fn icon(self) -> &'static str {
		match self {
			Self::Overview => "settings",
			Self::Roles => "shield",
			Self::Emoji => "emoji",
			Self::Stickers => "sticker",
			Self::Members => "members",
			Self::Invites => "mail",
			Self::Bans => "ban",
			Self::AuditLog => "audit_log",
		}
	}

	pub fn allowed(self, access: &GuildAccess) -> bool {
		match self {
			Self::Overview | Self::Invites => access.has(Permissions::MANAGE_GUILD),
			Self::Roles => access.has(Permissions::MANAGE_ROLES),
			Self::Emoji | Self::Stickers => {
				access.has_any(Permissions::MANAGE_GUILD_EXPRESSIONS | Permissions::CREATE_GUILD_EXPRESSIONS)
			}
			Self::Members => access.has_any(
				Permissions::MANAGE_GUILD
					| Permissions::MANAGE_ROLES
					| Permissions::KICK_MEMBERS
					| Permissions::BAN_MEMBERS
					| Permissions::MODERATE_MEMBERS
					| Permissions::MANAGE_NICKNAMES,
			),
			Self::Bans => access.has(Permissions::BAN_MEMBERS),
			Self::AuditLog => access.has(Permissions::VIEW_AUDIT_LOG),
		}
	}

	pub fn opening(wanted: Option<Self>, access: &GuildAccess) -> Option<Self> {
		wanted
			.filter(|page| page.allowed(access))
			.or_else(|| Self::ALL.into_iter().find(|page| page.allowed(access)))
	}
}

pub fn role_color(role: &Role) -> u32 {
	let value = if role.color != 0 {
		role.color
	} else {
		role.colors
			.as_ref()
			.and_then(|colors| colors.primary_color)
			.unwrap_or(0)
	};
	u32::try_from(value).unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRow {
	pub id: Snowflake,
	pub name: String,
	pub color: Option<u32>,
	pub members: usize,
	pub everyone: bool,
	pub movable: bool,
}

pub fn sorted_roles(state: &State, guild_id: Snowflake) -> Vec<Role> {
	let mut roles: Vec<Role> = state
		.guild(guild_id)
		.map(|guild| guild.roles.values().cloned().collect())
		.unwrap_or_default();
	roles.sort_by_key(|role| Reverse((role.id != guild_id, role.position, role.id)));
	roles
}

pub fn role_rows(state: &State, guild_id: Snowflake) -> Vec<RoleRow> {
	let access = GuildAccess::of(state, guild_id);
	let Some(guild) = state.guild(guild_id) else {
		return Vec::new();
	};
	sorted_roles(state, guild_id)
		.into_iter()
		.map(|role| {
			let everyone = role.id == guild_id;
			let members = if everyone {
				guild.members.len()
			} else {
				guild
					.members
					.iter()
					.filter(|(_, member)| member.roles.contains(&role.id))
					.count()
			};
			RoleRow {
				id: role.id,
				color: Some(role_color(&role)).filter(|color| *color != 0),
				members,
				everyone,
				movable: !everyone && access.above(role.position),
				name: role.name,
			}
		})
		.collect()
}

pub fn role_move_to(
	state: &State,
	guild_id: Snowflake,
	role_id: Snowflake,
	onto: Snowflake,
) -> Option<Vec<(Snowflake, i64)>> {
	let access = GuildAccess::of(state, guild_id);
	let roles = sorted_roles(state, guild_id);
	let from = roles.iter().position(|role| role.id == role_id)?;
	let to = roles.iter().position(|role| role.id == onto)?;
	let (low, high) = (from.min(to), from.max(to));
	let span = &roles[low..=high];
	let movable = |role: &Role| role.id != guild_id && access.above(role.position);
	if from == to || !span.iter().all(movable) {
		return None;
	}
	let mut order: Vec<&Role> = span.iter().collect();
	let moved = order.remove(from - low);
	order.insert(to - low, moved);
	Some(
		order
			.into_iter()
			.zip(span.iter().map(|role| role.position))
			.filter(|(role, position)| role.position != *position)
			.map(|(role, position)| (role.id, position))
			.collect(),
	)
}

pub struct Right {
	pub flag: Permissions,
	pub name: &'static str,
	pub hint: &'static str,
}

pub struct RightGroup {
	pub title: &'static str,
	pub rights: &'static [Right],
}

const fn right(flag: Permissions, name: &'static str, hint: &'static str) -> Right {
	Right { flag, name, hint }
}

pub const RIGHT_GROUPS: &[RightGroup] = &[
	RightGroup {
		title: "General Server Permissions",
		rights: &[
			right(
				Permissions::VIEW_CHANNEL,
				"View Channels",
				"Members can view channels by default (excluding private channels).",
			),
			right(
				Permissions::MANAGE_CHANNELS,
				"Manage Channels",
				"Create, edit, or delete channels.",
			),
			right(
				Permissions::MANAGE_ROLES,
				"Manage Roles",
				"Create new roles and edit or delete roles lower than their highest role.",
			),
			right(
				Permissions::CREATE_GUILD_EXPRESSIONS,
				"Create Expressions",
				"Add custom emoji, stickers, and sounds in this server.",
			),
			right(
				Permissions::MANAGE_GUILD_EXPRESSIONS,
				"Manage Expressions",
				"Edit or remove custom emoji, stickers, and sounds in this server.",
			),
			right(
				Permissions::VIEW_AUDIT_LOG,
				"View Audit Log",
				"Members can view a log of who made which changes in this server.",
			),
			right(
				Permissions::MANAGE_WEBHOOKS,
				"Manage Webhooks",
				"Create, edit, or delete webhooks.",
			),
			right(
				Permissions::MANAGE_GUILD,
				"Manage Server",
				"Change the server name, region, icon, and other server settings.",
			),
		],
	},
	RightGroup {
		title: "Membership Permissions",
		rights: &[
			right(
				Permissions::CREATE_INSTANT_INVITE,
				"Create Invite",
				"Invite new people to this server.",
			),
			right(
				Permissions::CHANGE_NICKNAME,
				"Change Nickname",
				"Change their own nickname.",
			),
			right(
				Permissions::MANAGE_NICKNAMES,
				"Manage Nicknames",
				"Change the nicknames of other members.",
			),
			right(
				Permissions::KICK_MEMBERS,
				"Kick Members",
				"Remove other members from this server.",
			),
			right(
				Permissions::BAN_MEMBERS,
				"Ban Members",
				"Permanently ban other members from this server.",
			),
			right(
				Permissions::MODERATE_MEMBERS,
				"Timeout Members",
				"Prevent members from sending messages, reacting, or speaking for a set period.",
			),
		],
	},
	RightGroup {
		title: "Text Channel Permissions",
		rights: &[
			right(
				Permissions::SEND_MESSAGES,
				"Send Messages",
				"Send messages in text channels.",
			),
			right(
				Permissions::SEND_MESSAGES_IN_THREADS,
				"Send Messages in Threads",
				"Send messages in threads.",
			),
			right(
				Permissions::CREATE_PUBLIC_THREADS,
				"Create Public Threads",
				"Create threads that everyone in a channel can view.",
			),
			right(
				Permissions::CREATE_PRIVATE_THREADS,
				"Create Private Threads",
				"Create invite-only threads.",
			),
			right(
				Permissions::EMBED_LINKS,
				"Embed Links",
				"Links shared by members show a preview.",
			),
			right(
				Permissions::ATTACH_FILES,
				"Attach Files",
				"Upload files or media.",
			),
			right(
				Permissions::ADD_REACTIONS,
				"Add Reactions",
				"Add new reactions to a message.",
			),
			right(
				Permissions::USE_EXTERNAL_EMOJIS,
				"Use External Emoji",
				"Use emoji from other servers.",
			),
			right(
				Permissions::USE_EXTERNAL_STICKERS,
				"Use External Stickers",
				"Use stickers from other servers.",
			),
			right(
				Permissions::MENTION_EVERYONE,
				"Mention @everyone, @here and All Roles",
				"Notify everyone with @everyone, @here, or any role.",
			),
			right(
				Permissions::MANAGE_MESSAGES,
				"Manage Messages",
				"Delete messages by other members or pin any message.",
			),
			right(
				Permissions::MANAGE_THREADS,
				"Manage Threads",
				"Rename, delete, close, and turn on slow mode for threads.",
			),
			right(
				Permissions::READ_MESSAGE_HISTORY,
				"Read Message History",
				"Read previous messages sent in channels.",
			),
			right(
				Permissions::SEND_TTS_MESSAGES,
				"Send Text-to-Speech Messages",
				"Send messages that are read aloud.",
			),
			right(
				Permissions::SEND_VOICE_MESSAGES,
				"Send Voice Messages",
				"Send voice messages.",
			),
			right(Permissions::SEND_POLLS, "Create Polls", "Create polls."),
		],
	},
	RightGroup {
		title: "Voice Channel Permissions",
		rights: &[
			right(Permissions::CONNECT, "Connect", "Join voice channels."),
			right(Permissions::SPEAK, "Speak", "Talk in voice channels."),
			right(Permissions::STREAM, "Video", "Share video or stream a screen."),
			right(
				Permissions::USE_SOUNDBOARD,
				"Use Soundboard",
				"Send sounds from the server soundboard.",
			),
			right(
				Permissions::USE_EXTERNAL_SOUNDS,
				"Use External Sounds",
				"Use sounds from other servers.",
			),
			right(
				Permissions::USE_VAD,
				"Use Voice Activity",
				"Speak without push-to-talk.",
			),
			right(
				Permissions::PRIORITY_SPEAKER,
				"Priority Speaker",
				"Be heard more clearly when talking.",
			),
			right(
				Permissions::MUTE_MEMBERS,
				"Mute Members",
				"Mute other members in voice channels.",
			),
			right(
				Permissions::DEAFEN_MEMBERS,
				"Deafen Members",
				"Deafen other members in voice channels.",
			),
			right(
				Permissions::MOVE_MEMBERS,
				"Move Members",
				"Move members between voice channels.",
			),
		],
	},
	RightGroup {
		title: "App Permissions",
		rights: &[
			right(
				Permissions::USE_APPLICATION_COMMANDS,
				"Use Application Commands",
				"Use commands from applications.",
			),
			right(
				Permissions::USE_EMBEDDED_ACTIVITIES,
				"Use Activities",
				"Use activities in voice channels.",
			),
			right(
				Permissions::USE_EXTERNAL_APPS,
				"Use External Apps",
				"Use apps that are not installed in this server.",
			),
		],
	},
	RightGroup {
		title: "Stage Channel Permissions",
		rights: &[right(
			Permissions::REQUEST_TO_SPEAK,
			"Request to Speak",
			"Ask to speak in stage channels.",
		)],
	},
	RightGroup {
		title: "Events Permissions",
		rights: &[
			right(Permissions::CREATE_EVENTS, "Create Events", "Create events."),
			right(
				Permissions::MANAGE_EVENTS,
				"Manage Events",
				"Edit and cancel events.",
			),
		],
	},
	RightGroup {
		title: "Advanced Permissions",
		rights: &[right(
			Permissions::ADMINISTRATOR,
			"Administrator",
			"Members with this permission have every permission and bypass channel overwrites.",
		)],
	},
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
	pub id: Snowflake,
	pub name: String,
	pub username: String,
	pub color: Option<u32>,
	pub roles: Vec<RoleChip>,
	pub joined: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemberSort {
	#[default]
	Newest,
	Oldest,
	Name,
}

pub const MEMBER_SORTS: [(MemberSort, &str); 3] = [
	(MemberSort::Newest, "Sort: Member Since"),
	(MemberSort::Oldest, "Sort: Oldest Members"),
	(MemberSort::Name, "Sort: Name"),
];

impl MemberSort {
	pub fn at(index: usize) -> Self {
		MEMBER_SORTS
			.get(index)
			.map_or_else(Self::default, |(sort, _)| *sort)
	}
}

pub fn member_rows(
	state: &State,
	guild_id: Snowflake,
	query: &str,
	sort: MemberSort,
) -> (Vec<MemberRow>, usize) {
	let Some(guild) = state.guild(guild_id) else {
		return (Vec::new(), 0);
	};
	let query = query.trim().to_lowercase();
	let mut rows: Vec<MemberRow> = guild
		.members
		.iter()
		.filter_map(|(id, member)| {
			let name = state.display_name(Some(guild_id), *id);
			let username = state
				.user(*id)
				.map(|user| user.username.clone())
				.unwrap_or_default();
			let matches = query.is_empty()
				|| name.to_lowercase().contains(&query)
				|| username.to_lowercase().contains(&query);
			matches.then(|| MemberRow {
				id: *id,
				color: state.role_color(guild_id, *id),
				roles: member
					.roles
					.iter()
					.filter_map(|role| guild.roles.get(role))
					.map(|role| RoleChip {
						name: role.name.clone(),
						color: Some(role_color(role)).filter(|color| *color != 0),
					})
					.collect(),
				joined: member.joined_at,
				name,
				username,
			})
		})
		.collect();
	let total = rows.len();
	match sort {
		MemberSort::Name => rows.sort_by_key(|row| row.name.to_lowercase()),
		MemberSort::Newest => rows.sort_by_key(|row| (Reverse(row.joined), row.name.to_lowercase())),
		MemberSort::Oldest => {
			rows.sort_by_key(|row| (row.joined.is_none(), row.joined, row.name.to_lowercase()));
		}
	}
	rows.truncate(MEMBER_ROW_LIMIT);
	(rows, total)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BanRow {
	pub id: Snowflake,
	pub name: String,
	pub username: String,
	pub reason: Option<String>,
}

pub fn ban_rows(state: &State, guild_id: Snowflake, query: &str) -> Vec<BanRow> {
	let query = query.trim().to_lowercase();
	state
		.bans(guild_id)
		.iter()
		.filter(|ban| {
			query.is_empty()
				|| ban.user.username.to_lowercase().contains(&query)
				|| ban
					.user
					.global_name
					.as_deref()
					.is_some_and(|name| name.to_lowercase().contains(&query))
				|| ban.user.id.to_string() == query
		})
		.map(|ban| BanRow {
			id: ban.user.id,
			name: ban
				.user
				.global_name
				.clone()
				.filter(|name| !name.is_empty())
				.unwrap_or_else(|| ban.user.username.clone()),
			username: ban.user.username.clone(),
			reason: ban.reason.clone(),
		})
		.collect()
}

pub const AUDIT_FILTERS: [(i64, &str); 22] = [
	(-1, "All Actions"),
	(1, "Server Update"),
	(10, "Channel Create"),
	(11, "Channel Update"),
	(12, "Channel Delete"),
	(20, "Member Kick"),
	(22, "Member Ban"),
	(23, "Member Unban"),
	(24, "Member Update"),
	(25, "Member Role Update"),
	(30, "Role Create"),
	(31, "Role Update"),
	(32, "Role Delete"),
	(40, "Invite Create"),
	(42, "Invite Delete"),
	(60, "Emoji Create"),
	(61, "Emoji Update"),
	(62, "Emoji Delete"),
	(72, "Message Delete"),
	(74, "Message Pin"),
	(75, "Message Unpin"),
	(110, "Thread Create"),
];

pub fn audit_filter(index: usize) -> Option<i64> {
	AUDIT_FILTERS
		.get(index)
		.map(|(kind, _)| *kind)
		.filter(|kind| *kind >= 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditChangeLine {
	pub key: String,
	pub old: String,
	pub new: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
	pub id: Snowflake,
	pub actor_id: Option<Snowflake>,
	pub actor: String,
	pub sentence: String,
	pub parts: Vec<(String, bool)>,
	pub icon: &'static str,
	pub tone: AuditTone,
	pub when: String,
	pub changes: Vec<AuditChangeLine>,
	pub reason: Option<String>,
}

fn shown(value: &Value) -> String {
	let text = match value {
		Value::String(text) => text.clone(),
		Value::Null => String::new(),
		Value::Array(items) => format!("[{}]", items.len()),
		other => other.to_string(),
	};
	if text.chars().count() > VALUE_LIMIT {
		let mut shortened: String = text.chars().take(VALUE_LIMIT).collect();
		shortened.push('…');
		shortened
	} else {
		text
	}
}

fn target_label(state: &State, guild_id: Snowflake, entry: &AuditEntry) -> String {
	let named = entry.changes.iter().find_map(|change| {
		matches!(change.key.as_str(), "name" | "code")
			.then(|| change.new.as_ref().or(change.old.as_ref()).map(shown))
			.flatten()
	});
	let Some(id) = entry.target_id else {
		return named.unwrap_or_default();
	};
	match entry.action_type {
		20 | 22 | 23 | 24 | 25 | 26 | 27 | 72 | 74 | 75 => state.display_name(Some(guild_id), id),
		10..=15 | 110..=112 => state
			.channel(id)
			.and_then(|channel| channel.name.clone())
			.or(named)
			.map_or_else(|| id.to_string(), |name| format!("#{name}")),
		30..=32 => state
			.guild(guild_id)
			.and_then(|guild| guild.roles.get(&id).map(|role| role.name.clone()))
			.or(named)
			.unwrap_or_else(|| id.to_string()),
		_ => named.unwrap_or_else(|| id.to_string()),
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditTone {
	Create,
	Update,
	Delete,
}

impl AuditTone {
	pub fn of(action_type: i64) -> Self {
		match action_type {
			10 | 13 | 23 | 30 | 40 | 50 | 60 | 74 | 110 => Self::Create,
			12 | 15 | 20 | 21 | 22 | 27 | 32 | 42 | 52 | 62 | 72 | 73 | 75 | 112 => Self::Delete,
			_ => Self::Update,
		}
	}

	pub fn key(self) -> &'static str {
		match self {
			Self::Create => "create",
			Self::Update => "update",
			Self::Delete => "delete",
		}
	}
}

pub fn audit_icon(action_type: i64) -> &'static str {
	match action_type {
		1 => "settings",
		10..=15 => "hash",
		20 | 21 => "user_x",
		22 | 23 => "gavel",
		24..=27 => "members",
		30..=32 => "shield",
		40..=42 => "link",
		50..=52 => "connections",
		60..=62 => "emoji",
		72..=75 => "chat",
		110..=112 => "thread",
		_ => "audit_log",
	}
}

const ACTOR_MARK: char = '\u{1}';
const TARGET_MARK: char = '\u{2}';

pub fn audit_parts(action_type: i64, actor: &str, target: &str) -> Vec<(String, bool)> {
	let template = audit_sentence(action_type, &ACTOR_MARK.to_string(), &TARGET_MARK.to_string());
	let mut parts = Vec::new();
	let mut plain = String::new();
	for character in template.chars() {
		let named = match character {
			ACTOR_MARK => actor,
			TARGET_MARK => target,
			_ => {
				plain.push(character);
				continue;
			}
		};
		if !plain.is_empty() {
			parts.push((std::mem::take(&mut plain), false));
		}
		if !named.is_empty() {
			parts.push((named.to_owned(), true));
		}
	}
	if !plain.is_empty() {
		parts.push((plain, false));
	}
	parts
}

pub fn audit_sentence(action_type: i64, actor: &str, target: &str) -> String {
	match action_type {
		1 => format!("{actor} changed the server settings"),
		10 => format!("{actor} created the channel {target}"),
		11 => format!("{actor} updated the channel {target}"),
		12 => format!("{actor} deleted the channel {target}"),
		13 => format!("{actor} created a permission overwrite on {target}"),
		14 => format!("{actor} updated a permission overwrite on {target}"),
		15 => format!("{actor} deleted a permission overwrite on {target}"),
		20 => format!("{actor} kicked {target}"),
		21 => format!("{actor} pruned inactive members"),
		22 => format!("{actor} banned {target}"),
		23 => format!("{actor} unbanned {target}"),
		24 => format!("{actor} updated {target}"),
		25 => format!("{actor} changed the roles of {target}"),
		26 => format!("{actor} moved {target} to another voice channel"),
		27 => format!("{actor} disconnected {target} from voice"),
		30 => format!("{actor} created the role {target}"),
		31 => format!("{actor} updated the role {target}"),
		32 => format!("{actor} deleted the role {target}"),
		40 => format!("{actor} created the invite {target}"),
		41 => format!("{actor} updated the invite {target}"),
		42 => format!("{actor} deleted the invite {target}"),
		50 => format!("{actor} created a webhook"),
		51 => format!("{actor} updated a webhook"),
		52 => format!("{actor} deleted a webhook"),
		60 => format!("{actor} created the emoji {target}"),
		61 => format!("{actor} updated the emoji {target}"),
		62 => format!("{actor} deleted the emoji {target}"),
		72 => format!("{actor} deleted a message by {target}"),
		73 => format!("{actor} deleted several messages"),
		74 => format!("{actor} pinned a message by {target}"),
		75 => format!("{actor} unpinned a message by {target}"),
		110 => format!("{actor} created the thread {target}"),
		111 => format!("{actor} updated the thread {target}"),
		112 => format!("{actor} deleted the thread {target}"),
		_ => format!("{actor} did something"),
	}
}

pub fn audit_rows(state: &State, guild_id: Snowflake, today: NaiveDate) -> Option<Vec<AuditRow>> {
	let log = state.audit_log(guild_id)?;
	Some(
		log.entries
			.iter()
			.map(|entry| {
				let actor = entry
					.user_id
					.map(|id| state.display_name(Some(guild_id), id))
					.unwrap_or_default();
				let target = target_label(state, guild_id, entry);
				AuditRow {
					id: entry.id,
					actor_id: entry.user_id,
					sentence: audit_sentence(entry.action_type, &actor, &target),
					parts: audit_parts(entry.action_type, &actor, &target),
					icon: audit_icon(entry.action_type),
					tone: AuditTone::of(entry.action_type),
					when: message_time(entry.id.timestamp(), today),
					changes: entry
						.changes
						.iter()
						.take(CHANGE_LIMIT)
						.map(|change| AuditChangeLine {
							key: change.key.clone(),
							old: change.old.as_ref().map(shown).unwrap_or_default(),
							new: change.new.as_ref().map(shown).unwrap_or_default(),
						})
						.collect(),
					reason: entry.reason.clone(),
					actor,
				}
			})
			.collect(),
	)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleChoice {
	pub id: Snowflake,
	pub name: String,
	pub held: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChoice {
	pub id: Snowflake,
	pub name: String,
	pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationItem {
	Separator,
	Nickname,
	Roles(Vec<RoleChoice>),
	MoveTo(Vec<VoiceChoice>),
	ServerMute(bool),
	ServerDeafen(bool),
	Timeout,
	RemoveTimeout,
	Kick,
	Ban,
}

impl ModerationItem {
	pub fn label(&self) -> &'static str {
		match self {
			Self::Separator => "",
			Self::Nickname => "Change Nickname",
			Self::Roles(_) => "Roles",
			Self::MoveTo(_) => "Move To",
			Self::ServerMute(true) => "Server Unmute",
			Self::ServerMute(false) => "Server Mute",
			Self::ServerDeafen(true) => "Server Undeafen",
			Self::ServerDeafen(false) => "Server Deafen",
			Self::Timeout => "Timeout",
			Self::RemoveTimeout => "Remove Timeout",
			Self::Kick => "Kick",
			Self::Ban => "Ban",
		}
	}

	pub fn icon(&self) -> &'static str {
		match self {
			Self::Separator => "",
			Self::Nickname => "edit",
			Self::Roles(_) => "shield",
			Self::MoveTo(_) => "volume",
			Self::ServerMute(true) => "mic",
			Self::ServerMute(false) => "mic_off",
			Self::ServerDeafen(true) => "headphones",
			Self::ServerDeafen(false) => "headphones_off",
			Self::Timeout | Self::RemoveTimeout => "timer",
			Self::Kick => "user_x",
			Self::Ban => "ban",
		}
	}

	pub fn is_danger(&self) -> bool {
		matches!(self, Self::Kick | Self::Ban)
	}
}

pub fn moderation_items(
	state: &State,
	guild_id: Snowflake,
	target: Snowflake,
	now: DateTime<Utc>,
) -> Vec<ModerationItem> {
	let (Some(me), Some(guild)) = (state.current_user_id(), state.guild(guild_id)) else {
		return Vec::new();
	};
	let access = GuildAccess::of(state, guild_id);
	let own = me == target;
	let outranks = !own && state.outranks(guild_id, me, target);
	let member = guild.members.get(&target);
	let timed_out = member
		.and_then(|member| member.communication_disabled_until)
		.is_some_and(|until| until > now);
	let mut items = Vec::new();
	let can_nick = if own {
		access.has(Permissions::CHANGE_NICKNAME)
	} else {
		outranks && access.has(Permissions::MANAGE_NICKNAMES)
	};
	if can_nick {
		items.push(ModerationItem::Nickname);
	}
	if (outranks || own && access.owner) && access.has(Permissions::MANAGE_ROLES) {
		let held = member.map(|member| member.roles.clone()).unwrap_or_default();
		let choices: Vec<RoleChoice> = sorted_roles(state, guild_id)
			.into_iter()
			.filter(|role| role.id != guild_id && !role.managed && access.above(role.position))
			.map(|role| RoleChoice {
				id: role.id,
				held: held.contains(&role.id),
				name: role.name,
			})
			.collect();
		if !choices.is_empty() {
			items.push(ModerationItem::Roles(choices));
		}
	}
	if let Some(voice) = guild
		.voice_states
		.get(&target)
		.filter(|voice| voice.channel_id.is_some())
	{
		if outranks && access.has(Permissions::MOVE_MEMBERS) {
			let channels = state
				.guild_channel_tree(guild_id)
				.into_iter()
				.flat_map(|node| node.children)
				.filter(|channel| {
					!channel.locked
						&& matches!(
							channel.kind,
							ChannelType::GuildVoice | ChannelType::GuildStageVoice
						)
				})
				.map(|channel| VoiceChoice {
					current: voice.channel_id == Some(channel.id),
					id: channel.id,
					name: channel.name,
				})
				.collect();
			items.push(ModerationItem::MoveTo(channels));
		}
		if outranks && access.has(Permissions::MUTE_MEMBERS) {
			items.push(ModerationItem::ServerMute(voice.mute));
		}
		if outranks && access.has(Permissions::DEAFEN_MEMBERS) {
			items.push(ModerationItem::ServerDeafen(voice.deaf));
		}
	}
	let mut punish = Vec::new();
	if outranks && access.has(Permissions::MODERATE_MEMBERS) {
		punish.push(if timed_out {
			ModerationItem::RemoveTimeout
		} else {
			ModerationItem::Timeout
		});
	}
	if outranks && access.has(Permissions::KICK_MEMBERS) {
		punish.push(ModerationItem::Kick);
	}
	if outranks && access.has(Permissions::BAN_MEMBERS) {
		punish.push(ModerationItem::Ban);
	}
	if !punish.is_empty() {
		if !items.is_empty() {
			items.push(ModerationItem::Separator);
		}
		items.extend(punish);
	}
	items
}

pub const TIMEOUT_CHOICES: [(&str, i64); 6] = [
	("60 secs", 60),
	("5 mins", 300),
	("10 mins", 600),
	("1 hour", 3600),
	("1 day", 86_400),
	("1 week", 604_800),
];
pub const DEFAULT_TIMEOUT: usize = 2;

pub const BAN_HISTORY_CHOICES: [(&str, u32); 7] = [
	("Do not delete any", 0),
	("Previous hour", 3600),
	("Previous 6 hours", 21_600),
	("Previous 12 hours", 43_200),
	("Previous 24 hours", 86_400),
	("Previous 3 days", 259_200),
	("Previous 7 days", 604_800),
];

pub const NICKNAME_LIMIT: usize = 32;
pub const REASON_LIMIT: usize = 512;

pub fn trimmed(text: &str, limit: usize) -> Option<String> {
	let text: String = text.trim().chars().take(limit).collect();
	(!text.is_empty()).then_some(text)
}
