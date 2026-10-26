use chrono::{DateTime, Utc};
use markdown::TimestampStyle;
use protocol::{
	Channel, ChannelType, MEMBER_QUARANTINED_USERNAME, PermissionOverwrite, PermissionOverwriteKind,
	Permissions, Snowflake,
};
use state::{GuildState, State};

use crate::guild_admin::role_color;
use crate::message::{UNKNOWN_ROLE, timestamp_text};
use crate::sidebar::channel_icon;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overwriter {
	pub id: Snowflake,
	pub name: String,
	pub member: bool,
	pub color: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenChannel {
	pub id: Snowflake,
	pub guild_id: Snowflake,
	pub name: String,
	pub kind: &'static str,
	pub icon: &'static str,
	pub topic: Option<String>,
	pub nsfw: bool,
	pub slowmode: Option<u32>,
	pub created_at: DateTime<Utc>,
	pub last_message_at: Option<DateTime<Utc>>,
	pub allowed: Vec<Overwriter>,
	pub denied: Vec<Overwriter>,
}

impl HiddenChannel {
	pub fn activity_text(&self, now: DateTime<Utc>) -> String {
		match self.last_message_at {
			Some(at) => format!(
				"Last message {}, {}",
				timestamp_text(at.timestamp(), TimestampStyle::Relative, now),
				timestamp_text(at.timestamp(), TimestampStyle::ShortDateTime, now),
			),
			None => "No messages yet".to_owned(),
		}
	}

	pub fn created_text(&self, now: DateTime<Utc>) -> String {
		format!(
			"Created {}",
			timestamp_text(self.created_at.timestamp(), TimestampStyle::LongDate, now)
		)
	}

	pub fn slowmode_text(&self) -> Option<String> {
		let seconds = self.slowmode.filter(|seconds| *seconds > 0)?;
		Some(match seconds {
			seconds if seconds % 3600 == 0 => format!("Slowmode {}h", seconds / 3600),
			seconds if seconds % 60 == 0 => format!("Slowmode {}m", seconds / 60),
			seconds => format!("Slowmode {seconds}s"),
		})
	}
}

pub fn channel_kind_label(kind: ChannelType) -> &'static str {
	match kind {
		ChannelType::GuildText => "Text Channel",
		ChannelType::GuildAnnouncement => "Announcement Channel",
		ChannelType::GuildVoice => "Voice Channel",
		ChannelType::GuildStageVoice => "Stage Channel",
		ChannelType::GuildForum => "Forum Channel",
		ChannelType::GuildMedia => "Media Channel",
		ChannelType::GuildCategory => "Category",
		ChannelType::AnnouncementThread | ChannelType::PublicThread | ChannelType::PrivateThread => "Thread",
		_ => "Channel",
	}
}

pub fn hidden_channel(state: &State, channel_id: Snowflake) -> Option<HiddenChannel> {
	if !state.is_hidden(channel_id) {
		return None;
	}
	let guild_id = state.channel_guild(channel_id)?;
	let guild = state.guild(guild_id)?;
	let channel = guild.channel(channel_id)?;
	let source = overwrite_source(guild, channel);
	let (allowed, denied) = view_overwrites(state, guild, &source.permission_overwrites);
	Some(HiddenChannel {
		id: channel_id,
		guild_id,
		name: channel.name.clone().unwrap_or_default(),
		kind: channel_kind_label(channel.kind),
		icon: channel_icon(channel.kind),
		topic: channel
			.topic
			.as_deref()
			.map(str::trim)
			.filter(|topic| !topic.is_empty())
			.map(str::to_owned),
		nsfw: channel.nsfw == Some(true),
		slowmode: channel
			.rate_limit_per_user
			.and_then(|seconds| u32::try_from(seconds).ok()),
		created_at: channel_id.timestamp(),
		last_message_at: channel.last_message_id.map(Snowflake::timestamp),
		allowed,
		denied,
	})
}

fn overwrite_source<'a>(guild: &'a GuildState, channel: &'a Channel) -> &'a Channel {
	let threaded = matches!(
		channel.kind,
		ChannelType::AnnouncementThread | ChannelType::PublicThread | ChannelType::PrivateThread
	);
	if threaded && let Some(parent) = channel.parent_id.and_then(|id| guild.channel(id)) {
		return parent;
	}
	channel
}

fn view_overwrites(
	state: &State,
	guild: &GuildState,
	overwrites: &[PermissionOverwrite],
) -> (Vec<Overwriter>, Vec<Overwriter>) {
	let mut allowed = Vec::new();
	let mut denied = Vec::new();
	for overwrite in overwrites {
		let list = if overwrite.allow.contains(Permissions::VIEW_CHANNEL) {
			&mut allowed
		} else if overwrite.deny.contains(Permissions::VIEW_CHANNEL) {
			&mut denied
		} else {
			continue;
		};
		list.push(overwriter(state, guild, overwrite));
	}
	for list in [&mut allowed, &mut denied] {
		list.sort_by_cached_key(|entry| {
			let position = if entry.member {
				i64::MIN
			} else {
				guild.roles.get(&entry.id).map_or(0, |role| role.position)
			};
			(
				entry.member,
				std::cmp::Reverse(position),
				entry.name.to_lowercase(),
			)
		});
	}
	(allowed, denied)
}

fn overwriter(state: &State, guild: &GuildState, overwrite: &PermissionOverwrite) -> Overwriter {
	match overwrite.kind {
		PermissionOverwriteKind::Member => Overwriter {
			id: overwrite.id,
			name: state.display_name(Some(guild.id), overwrite.id),
			member: true,
			color: state.role_color(guild.id, overwrite.id),
		},
		_ => {
			let role = guild.roles.get(&overwrite.id);
			Overwriter {
				id: overwrite.id,
				name: role.map_or_else(|| UNKNOWN_ROLE.to_owned(), |role| role.name.clone()),
				member: false,
				color: role.map(role_color).filter(|color| *color != 0),
			}
		}
	}
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeratorNotes {
	pub timed_out_until: Option<DateTime<Utc>>,
	pub pending: bool,
	pub quarantined: bool,
}

impl ModeratorNotes {
	pub fn is_empty(&self) -> bool {
		self.timed_out_until.is_none() && !self.pending && !self.quarantined
	}

	pub fn timeout_text(&self, now: DateTime<Utc>) -> Option<String> {
		let until = self.timed_out_until?;
		Some(format!(
			"Timed out until {}",
			timestamp_text(until.timestamp(), TimestampStyle::ShortDateTime, now)
		))
	}

	pub fn flag_text(&self) -> Option<String> {
		let parts: Vec<&str> = [
			(self.pending, "Pending membership screening"),
			(self.quarantined, "Quarantined by AutoMod"),
		]
		.into_iter()
		.filter_map(|(on, text)| on.then_some(text))
		.collect();
		(!parts.is_empty()).then(|| parts.join(" · "))
	}
}

pub fn moderator_notes(
	state: &State,
	guild_id: Snowflake,
	user_id: Snowflake,
	now: DateTime<Utc>,
) -> ModeratorNotes {
	let Some(member) = state
		.guild(guild_id)
		.and_then(|guild| guild.members.get(&user_id))
	else {
		return ModeratorNotes::default();
	};
	ModeratorNotes {
		timed_out_until: member.communication_disabled_until.filter(|until| *until > now),
		pending: member.pending == Some(true),
		quarantined: member
			.flags
			.is_some_and(|flags| flags & MEMBER_QUARANTINED_USERNAME != 0),
	}
}
