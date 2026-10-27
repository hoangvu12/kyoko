use std::cmp::Reverse;

use protocol::{Channel, ChannelFlags, ChannelType, PermissionOverwriteKind, Permissions, Snowflake};
use session::ChannelChanges;
use state::{
	BITRATE_MIN, CHANNEL_NAME_MAX, CHANNEL_TOPIC_MAX, InviteInfo, State, VIDEO_QUALITY_AUTO, bitrate_limit,
	user_limit_range,
};

use crate::guild_admin::{RIGHT_GROUPS, Right, role_color};
use crate::sidebar::channel_icon;

pub const SLOWMODE_CHOICES: [(u32, &str); 14] = [
	(0, "Off"),
	(5, "5s"),
	(10, "10s"),
	(15, "15s"),
	(30, "30s"),
	(60, "1m"),
	(120, "2m"),
	(300, "5m"),
	(600, "10m"),
	(900, "15m"),
	(1_800, "30m"),
	(3_600, "1h"),
	(7_200, "2h"),
	(21_600, "6h"),
];

pub const CANDIDATE_LIMIT: usize = 25;
const DEFAULT_ARCHIVE: u32 = 4_320;
const DEFAULT_BITRATE: u32 = 64_000;
const NEWS: &str = "NEWS";

const GUILD_ONLY: Permissions = Permissions::CREATE_GUILD_EXPRESSIONS
	.union(Permissions::MANAGE_GUILD_EXPRESSIONS)
	.union(Permissions::VIEW_AUDIT_LOG)
	.union(Permissions::MANAGE_GUILD)
	.union(Permissions::CHANGE_NICKNAME)
	.union(Permissions::MANAGE_NICKNAMES)
	.union(Permissions::KICK_MEMBERS)
	.union(Permissions::BAN_MEMBERS)
	.union(Permissions::MODERATE_MEMBERS)
	.union(Permissions::ADMINISTRATOR)
	.union(Permissions::CREATE_EVENTS)
	.union(Permissions::MANAGE_EVENTS);

const AUDIO_ONLY: Permissions = Permissions::CONNECT
	.union(Permissions::SPEAK)
	.union(Permissions::STREAM)
	.union(Permissions::USE_SOUNDBOARD)
	.union(Permissions::USE_EXTERNAL_SOUNDS)
	.union(Permissions::USE_VAD)
	.union(Permissions::PRIORITY_SPEAKER)
	.union(Permissions::MUTE_MEMBERS)
	.union(Permissions::DEAFEN_MEMBERS)
	.union(Permissions::MOVE_MEMBERS)
	.union(Permissions::USE_EMBEDDED_ACTIVITIES);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelPage {
	Overview,
	Permissions,
	Invites,
}

impl ChannelPage {
	pub const ALL: [ChannelPage; 3] = [Self::Overview, Self::Permissions, Self::Invites];

	pub fn key(self) -> &'static str {
		match self {
			Self::Overview => "overview",
			Self::Permissions => "permissions",
			Self::Invites => "invites",
		}
	}

	pub fn named(key: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|page| page.key() == key)
	}

	pub fn title(self) -> &'static str {
		match self {
			Self::Overview => "Overview",
			Self::Permissions => "Permissions",
			Self::Invites => "Invites",
		}
	}

	pub fn icon(self) -> &'static str {
		match self {
			Self::Overview => "settings",
			Self::Permissions => "shield",
			Self::Invites => "mail",
		}
	}

	pub fn allowed(self, access: ChannelAccess) -> bool {
		match self {
			Self::Overview | Self::Invites => access.manage_channels,
			Self::Permissions => access.manage_channels && access.manage_roles,
		}
	}
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChannelAccess {
	pub manage_channels: bool,
	pub manage_roles: bool,
	pub held: Permissions,
}

impl ChannelAccess {
	pub fn of(state: &State, channel_id: Snowflake) -> Self {
		if state.channel_guild(channel_id).is_none() {
			return Self::default();
		}
		let held = state.my_permissions(channel_id);
		let admin = held.contains(Permissions::ADMINISTRATOR);
		Self {
			manage_channels: admin || held.contains(Permissions::MANAGE_CHANNELS),
			manage_roles: admin || held.contains(Permissions::MANAGE_ROLES),
			held: if admin { Permissions::all() } else { held },
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelForm {
	pub name: String,
	pub topic: String,
	pub slowmode: u32,
	pub nsfw: bool,
	pub spoiler: bool,
	pub announcement: bool,
	pub auto_archive: u32,
	pub bitrate: u32,
	pub user_limit: u32,
	pub region: String,
	pub video_quality: i64,
	pub parent_id: Option<Snowflake>,
}

fn whole(value: Option<i64>, fallback: u32) -> u32 {
	value
		.and_then(|value| u32::try_from(value).ok())
		.unwrap_or(fallback)
}

impl ChannelForm {
	pub fn of(channel: &Channel) -> Self {
		Self {
			name: channel.name.clone().unwrap_or_default(),
			topic: channel.topic.clone().unwrap_or_default(),
			slowmode: whole(channel.rate_limit_per_user, 0),
			nsfw: channel.nsfw == Some(true),
			spoiler: channel.flags.contains(ChannelFlags::IS_SPOILER_CHANNEL),
			announcement: channel.kind == ChannelType::GuildAnnouncement,
			auto_archive: whole(channel.default_auto_archive_duration, DEFAULT_ARCHIVE),
			bitrate: whole(channel.bitrate, DEFAULT_BITRATE),
			user_limit: whole(channel.user_limit, 0),
			region: channel.rtc_region.clone().unwrap_or_default(),
			video_quality: channel.video_quality_mode.unwrap_or(VIDEO_QUALITY_AUTO),
			parent_id: channel.parent_id,
		}
	}

	pub fn changes(&self, wanted: &ChannelForm) -> ChannelChanges {
		fn changed<T: PartialEq + Clone>(old: &T, new: &T) -> Option<T> {
			(old != new).then(|| new.clone())
		}
		let name: String = wanted.name.trim().chars().take(CHANNEL_NAME_MAX).collect();
		let topic: String = wanted.topic.trim().chars().take(CHANNEL_TOPIC_MAX).collect();
		ChannelChanges {
			name: changed(&self.name, &name).filter(|name| !name.is_empty()),
			topic: changed(&self.topic, &topic),
			nsfw: changed(&self.nsfw, &wanted.nsfw),
			slowmode: changed(&self.slowmode, &wanted.slowmode),
			position: None,
			parent_id: changed(&self.parent_id, &wanted.parent_id).flatten(),
			announcement: changed(&self.announcement, &wanted.announcement),
			spoiler: changed(&self.spoiler, &wanted.spoiler),
			bitrate: changed(&self.bitrate, &wanted.bitrate),
			user_limit: changed(&self.user_limit, &wanted.user_limit),
			rtc_region: changed(&self.region, &wanted.region)
				.map(|region| Some(region).filter(|region| !region.is_empty())),
			video_quality: changed(&self.video_quality, &wanted.video_quality),
			auto_archive: changed(&self.auto_archive, &wanted.auto_archive),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSettings {
	pub id: Snowflake,
	pub guild_id: Snowflake,
	pub kind: ChannelType,
	pub label: &'static str,
	pub icon: &'static str,
	pub form: ChannelForm,
	pub audio: bool,
	pub topic: bool,
	pub archives: bool,
	pub announces: bool,
	pub bitrate_min: u32,
	pub bitrate_max: u32,
	pub user_limit_max: u32,
	pub categories: Vec<(Snowflake, String)>,
	pub pages: Vec<ChannelPage>,
}

pub fn short_kind_label(kind: ChannelType) -> &'static str {
	match kind {
		ChannelType::GuildAnnouncement => "Announcement",
		ChannelType::GuildVoice => "Voice",
		ChannelType::GuildStageVoice => "Stage",
		ChannelType::GuildForum => "Forum",
		ChannelType::GuildMedia => "Media",
		ChannelType::GuildCategory => "Category",
		_ => "Text",
	}
}

fn editable(kind: ChannelType) -> bool {
	matches!(
		kind,
		ChannelType::GuildText
			| ChannelType::GuildAnnouncement
			| ChannelType::GuildVoice
			| ChannelType::GuildStageVoice
			| ChannelType::GuildForum
			| ChannelType::GuildMedia
			| ChannelType::GuildCategory
	)
}

pub fn channel_settings(state: &State, channel_id: Snowflake) -> Option<ChannelSettings> {
	let guild_id = state.channel_guild(channel_id)?;
	let guild = state.guild(guild_id)?;
	let channel = guild.channel(channel_id)?;
	let access = ChannelAccess::of(state, channel_id);
	if !access.manage_channels || !editable(channel.kind) {
		return None;
	}
	let kind = channel.kind;
	let audio = matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice);
	let mut categories: Vec<(i64, Snowflake, String)> = guild
		.channels
		.values()
		.filter(|found| found.kind == ChannelType::GuildCategory)
		.map(|found| {
			(
				found.position.unwrap_or(0),
				found.id,
				found.name.clone().unwrap_or_default(),
			)
		})
		.collect();
	categories.sort();
	let (_, user_limit_max) = user_limit_range(kind);
	Some(ChannelSettings {
		id: channel_id,
		guild_id,
		kind,
		label: short_kind_label(kind),
		icon: channel_icon(kind),
		form: ChannelForm::of(channel),
		audio,
		topic: matches!(
			kind,
			ChannelType::GuildText
				| ChannelType::GuildAnnouncement
				| ChannelType::GuildForum
				| ChannelType::GuildMedia
		),
		archives: matches!(
			kind,
			ChannelType::GuildText
				| ChannelType::GuildAnnouncement
				| ChannelType::GuildForum
				| ChannelType::GuildMedia
		),
		announces: matches!(kind, ChannelType::GuildText | ChannelType::GuildAnnouncement)
			&& guild.features.iter().any(|feature| feature == NEWS),
		bitrate_min: BITRATE_MIN,
		bitrate_max: bitrate_limit(guild.premium_tier, &guild.features, kind),
		user_limit_max,
		categories: if kind == ChannelType::GuildCategory {
			Vec::new()
		} else {
			categories.into_iter().map(|(_, id, name)| (id, name)).collect()
		},
		pages: ChannelPage::ALL
			.into_iter()
			.filter(|page| page.allowed(access))
			.collect(),
	})
}

pub fn right_flags() -> Vec<Permissions> {
	RIGHT_GROUPS
		.iter()
		.flat_map(|group| group.rights.iter().map(|right| right.flag))
		.collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelRight {
	pub index: usize,
	pub name: &'static str,
	pub hint: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelRightGroup {
	pub title: &'static str,
	pub rights: Vec<ChannelRight>,
}

fn channel_wording(right: &Right) -> (&'static str, &'static str) {
	if right.flag == Permissions::VIEW_CHANNEL {
		(
			"View Channel",
			"Members can see this channel and read its messages.",
		)
	} else if right.flag == Permissions::MANAGE_CHANNELS {
		(
			"Manage Channel",
			"Change this channel's name, topic and settings, or delete it.",
		)
	} else if right.flag == Permissions::MANAGE_ROLES {
		("Manage Permissions", "Change this channel's permissions.")
	} else {
		(right.name, right.hint)
	}
}

fn channel_group_title(title: &'static str) -> &'static str {
	if title == "General Server Permissions" {
		"General Channel Permissions"
	} else {
		title
	}
}

pub fn channel_right_groups(kind: ChannelType) -> Vec<ChannelRightGroup> {
	let audio = matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice);
	let stage = kind == ChannelType::GuildStageVoice;
	let mut index = 0;
	let mut groups = Vec::new();
	for group in RIGHT_GROUPS {
		let mut rights = Vec::new();
		for right in group.rights {
			index += 1;
			let skipped = GUILD_ONLY.contains(right.flag)
				|| (!audio && AUDIO_ONLY.contains(right.flag))
				|| (!stage && right.flag == Permissions::REQUEST_TO_SPEAK);
			if !skipped {
				let (name, hint) = channel_wording(right);
				rights.push(ChannelRight { index, name, hint });
			}
		}
		if !rights.is_empty() {
			groups.push(ChannelRightGroup {
				title: channel_group_title(group.title),
				rights,
			});
		}
	}
	groups
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverwriteRow {
	pub id: Snowflake,
	pub member: bool,
	pub everyone: bool,
	pub name: String,
	pub color: Option<u32>,
	pub allow: Permissions,
	pub deny: Permissions,
	pub editable: bool,
}

pub fn overwrite_rows(state: &State, channel_id: Snowflake) -> Vec<OverwriteRow> {
	let Some(guild_id) = state.channel_guild(channel_id) else {
		return Vec::new();
	};
	let (Some(guild), Some(channel)) = (state.guild(guild_id), state.channel(channel_id)) else {
		return Vec::new();
	};
	let me = state.current_user_id();
	let owner = me.is_some() && guild.owner_id == me;
	let top = me.map_or(0, |me| state.top_role_position(guild_id, me));
	let mut roles = Vec::new();
	let mut members = Vec::new();
	let mut everyone = OverwriteRow {
		id: guild_id,
		member: false,
		everyone: true,
		name: "@everyone".to_owned(),
		color: None,
		allow: Permissions::empty(),
		deny: Permissions::empty(),
		editable: true,
	};
	for overwrite in channel.permission_overwrites.iter() {
		match overwrite.kind {
			PermissionOverwriteKind::Role if overwrite.id == guild_id => {
				everyone.allow = overwrite.allow;
				everyone.deny = overwrite.deny;
			}
			PermissionOverwriteKind::Role => {
				let Some(role) = guild.roles.get(&overwrite.id) else {
					continue;
				};
				roles.push((
					Reverse(role.position),
					OverwriteRow {
						id: role.id,
						member: false,
						everyone: false,
						name: role.name.clone(),
						color: Some(role_color(role)).filter(|color| *color != 0),
						allow: overwrite.allow,
						deny: overwrite.deny,
						editable: owner || role.position < top,
					},
				));
			}
			PermissionOverwriteKind::Member => members.push(OverwriteRow {
				id: overwrite.id,
				member: true,
				everyone: false,
				name: state.display_name(Some(guild_id), overwrite.id),
				color: state.role_color(guild_id, overwrite.id),
				allow: overwrite.allow,
				deny: overwrite.deny,
				editable: true,
			}),
			PermissionOverwriteKind::Unknown(_) => {}
		}
	}
	roles.sort_by_key(|(position, row)| (*position, row.id));
	members.sort_by_key(|row| (row.name.to_lowercase(), row.id));
	let mut rows = vec![everyone];
	rows.extend(roles.into_iter().map(|(_, row)| row));
	rows.extend(members);
	rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverwriteCandidate {
	pub id: Snowflake,
	pub member: bool,
	pub name: String,
	pub color: Option<u32>,
}

pub fn overwrite_candidates(state: &State, channel_id: Snowflake, query: &str) -> Vec<OverwriteCandidate> {
	let Some(guild_id) = state.channel_guild(channel_id) else {
		return Vec::new();
	};
	let (Some(guild), Some(channel)) = (state.guild(guild_id), state.channel(channel_id)) else {
		return Vec::new();
	};
	let taken = |id: Snowflake| {
		channel
			.permission_overwrites
			.iter()
			.any(|overwrite| overwrite.id == id)
	};
	let wanted = query.trim().to_lowercase();
	let matches = |name: &str| wanted.is_empty() || name.to_lowercase().contains(&wanted);
	let mut roles: Vec<_> = guild
		.roles
		.values()
		.filter(|role| role.id != guild_id && !taken(role.id) && matches(&role.name))
		.collect();
	roles.sort_by_key(|role| (Reverse(role.position), role.id));
	let mut found: Vec<OverwriteCandidate> = roles
		.into_iter()
		.map(|role| OverwriteCandidate {
			id: role.id,
			member: false,
			name: role.name.clone(),
			color: Some(role_color(role)).filter(|color| *color != 0),
		})
		.collect();
	let mut members: Vec<OverwriteCandidate> = guild
		.members
		.iter()
		.map(|(id, _)| *id)
		.filter(|id| !taken(*id))
		.map(|id| OverwriteCandidate {
			id,
			member: true,
			name: state.display_name(Some(guild_id), id),
			color: state.role_color(guild_id, id),
		})
		.filter(|candidate| matches(&candidate.name))
		.collect();
	members.sort_by_key(|candidate| (candidate.name.to_lowercase(), candidate.id));
	found.extend(members);
	found.truncate(CANDIDATE_LIMIT);
	found
}

pub fn overwrite_from(
	base: (Permissions, Permissions),
	choices: &[(usize, i8)],
) -> (Permissions, Permissions) {
	let flags = right_flags();
	let (mut allow, mut deny) = base;
	for (index, choice) in choices {
		let Some(flag) = index.checked_sub(1).and_then(|at| flags.get(at)) else {
			continue;
		};
		allow.remove(*flag);
		deny.remove(*flag);
		match choice.signum() {
			1 => allow.insert(*flag),
			-1 => deny.insert(*flag),
			_ => {}
		}
	}
	(allow, deny)
}

pub fn invite_uses(invite: &InviteInfo) -> String {
	if invite.max_uses > 0 {
		format!("{} / {}", invite.uses, invite.max_uses)
	} else {
		invite.uses.to_string()
	}
}
