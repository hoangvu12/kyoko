use chrono::{DateTime, Utc};
use protocol::{
	Channel, ChannelType, DefaultMessageNotifications, Message, MuteConfig, NotificationLevel, Permissions,
	ReadState, RelationshipKind, Snowflake, UserGuildSettings,
};

use crate::guild::GuildState;
use crate::permissions::compute_permissions;
use crate::state::State;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ChannelStatus {
	pub unread: bool,
	pub mentions: u32,
	pub muted: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct GuildStatus {
	pub unread: bool,
	pub mentions: u32,
	pub muted: bool,
}

pub(crate) fn mute_active(muted: bool, config: Option<&MuteConfig>, now: DateTime<Utc>) -> bool {
	muted
		&& config
			.and_then(|config| config.end_time)
			.is_none_or(|end| end > now)
}

pub(crate) fn guild_muted(settings: Option<&UserGuildSettings>, now: DateTime<Utc>) -> bool {
	settings.is_some_and(|settings| mute_active(settings.muted, settings.mute_config.as_ref(), now))
}

pub(crate) fn channel_muted(
	settings: Option<&UserGuildSettings>,
	channel: &Channel,
	now: DateTime<Utc>,
) -> bool {
	guild_muted(settings, now) || channel_override_muted(settings, channel, now)
}

fn channel_override_muted(
	settings: Option<&UserGuildSettings>,
	channel: &Channel,
	now: DateTime<Utc>,
) -> bool {
	let Some(settings) = settings else {
		return false;
	};
	[Some(channel.id), channel.parent_id]
		.into_iter()
		.flatten()
		.any(|id| {
			settings
				.channel_overrides
				.iter()
				.any(|over| over.channel_id == id && mute_active(over.muted, over.mute_config.as_ref(), now))
		})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
	DirectMessage,
	Mention,
	Message,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attention {
	pub unread: bool,
	pub mentions: u32,
}

fn explicit(level: Option<NotificationLevel>) -> Option<NotificationLevel> {
	level.filter(|level| {
		matches!(
			level,
			NotificationLevel::AllMessages | NotificationLevel::OnlyMentions | NotificationLevel::NoMessages
		)
	})
}

fn guild_default(guild: Option<&GuildState>) -> NotificationLevel {
	match guild.and_then(|guild| guild.default_notifications) {
		Some(DefaultMessageNotifications::AllMessages) => NotificationLevel::AllMessages,
		_ => NotificationLevel::OnlyMentions,
	}
}

fn override_level(
	settings: Option<&UserGuildSettings>,
	ids: [Option<Snowflake>; 2],
) -> Option<NotificationLevel> {
	let settings = settings?;
	ids.into_iter().flatten().find_map(|id| {
		settings
			.channel_overrides
			.iter()
			.find(|over| over.channel_id == id)
			.and_then(|over| explicit(over.message_notifications))
	})
}

fn mention_count(read_state: Option<&ReadState>) -> u32 {
	read_state
		.and_then(|state| state.mention_count)
		.map_or(0, |count| u32::try_from(count).unwrap_or(0))
}

impl State {
	pub fn alert_for(&self, message: &Message) -> Option<AlertKind> {
		self.alert_at(message, Utc::now())
	}

	fn alert_at(&self, message: &Message, now: DateTime<Utc>) -> Option<AlertKind> {
		let me = self.current_user_id()?;
		if message.author.id == me || self.is_blocked(message.author.id) {
			return None;
		}
		let channel = self.channel(message.channel_id);
		let guild_id = message
			.guild_id
			.or_else(|| self.channel_guild(message.channel_id));
		let settings = self.guild_settings(guild_id);
		let muted = match channel {
			Some(channel) => channel_muted(settings, channel, now),
			None => guild_muted(settings, now),
		};
		if muted {
			return None;
		}
		let ids = [
			Some(message.channel_id),
			channel.and_then(|channel| channel.parent_id),
		];
		let Some(guild_id) = guild_id else {
			return (override_level(settings, ids) != Some(NotificationLevel::NoMessages))
				.then_some(AlertKind::DirectMessage);
		};
		let level = override_level(settings, ids).unwrap_or_else(|| self.guild_notification_level(guild_id));
		let mentioned = self.mentions_me(message, Some(guild_id));
		match level {
			NotificationLevel::NoMessages => None,
			NotificationLevel::AllMessages if !mentioned => Some(AlertKind::Message),
			_ => mentioned.then_some(AlertKind::Mention),
		}
	}

	pub fn guild_notification_level(&self, guild_id: Snowflake) -> NotificationLevel {
		explicit(
			self.guild_settings(Some(guild_id))
				.and_then(|settings| settings.message_notifications),
		)
		.unwrap_or_else(|| guild_default(self.guild(guild_id)))
	}

	fn is_blocked(&self, user_id: Snowflake) -> bool {
		self.relationships
			.get(&user_id)
			.is_some_and(|relationship| relationship.kind == RelationshipKind::Blocked)
	}

	pub fn attention(&self) -> Attention {
		let now = Utc::now();
		let mut attention = Attention::default();
		for guild in self.guilds.values() {
			let status = self.guild_status(guild, now);
			attention.unread |= status.unread && !status.muted;
			attention.mentions += status.mentions;
		}
		let settings = self.global_settings.as_ref();
		for channel in self.private_channels.values() {
			let status = self.channel_status(channel, settings, true, now);
			attention.unread |= status.unread;
			attention.mentions += status.mentions;
		}
		attention
	}

	pub(crate) fn channel_status(
		&self,
		channel: &Channel,
		settings: Option<&UserGuildSettings>,
		viewable: bool,
		now: DateTime<Utc>,
	) -> ChannelStatus {
		let muted = channel_override_muted(settings, channel, now);
		if !viewable || channel.kind == ChannelType::GuildCategory {
			return ChannelStatus {
				muted,
				..ChannelStatus::default()
			};
		}
		let read_state = self.read_states.get(&channel.id);
		let behind = match (read_state, channel.last_message_id) {
			(Some(state), Some(last)) => last > state.last_message_id.unwrap_or_default(),
			_ => false,
		};
		ChannelStatus {
			unread: behind && !muted,
			mentions: mention_count(read_state),
			muted,
		}
	}

	pub(crate) fn guild_status(&self, guild: &GuildState, now: DateTime<Utc>) -> GuildStatus {
		let settings = self.guild_settings.get(&guild.id);
		let mut status = GuildStatus {
			muted: guild_muted(settings, now),
			..GuildStatus::default()
		};
		let Some(me) = self.current_user_id() else {
			return status;
		};
		for channel in guild.channels.values() {
			if channel.kind == ChannelType::GuildCategory {
				continue;
			}
			let viewable =
				compute_permissions(guild, Some(channel), me, now).contains(Permissions::VIEW_CHANNEL);
			let channel_status = self.channel_status(channel, settings, viewable, now);
			status.unread |= channel_status.unread;
			status.mentions += channel_status.mentions;
		}
		status
	}

	pub fn mentions_me(&self, message: &Message, guild_id: Option<Snowflake>) -> bool {
		let Some(me) = self.current_user_id() else {
			return false;
		};
		if message.mentions.iter().any(|user| user.id == me) {
			return true;
		}
		let settings = guild_id.and_then(|id| self.guild_settings.get(&id));
		if message.mention_everyone && !settings.is_some_and(|settings| settings.suppress_everyone) {
			return true;
		}
		if message.mention_roles.is_empty() || settings.is_some_and(|settings| settings.suppress_roles) {
			return false;
		}
		guild_id
			.and_then(|id| self.guilds.get(&id))
			.and_then(|guild| guild.members.get(&me))
			.is_some_and(|member| {
				member
					.roles
					.iter()
					.any(|role| message.mention_roles.contains(role))
			})
	}

	pub(crate) fn set_read_state(&mut self, channel_id: Snowflake, message_id: Snowflake, mentions: u32) {
		let state = self.read_states.entry(channel_id).or_insert_with(|| ReadState {
			id: channel_id,
			..ReadState::default()
		});
		state.last_message_id = Some(message_id);
		state.mention_count = Some(i64::from(mentions));
	}

	pub(crate) fn bump_mentions(&mut self, channel_id: Snowflake) {
		let state = self.read_states.entry(channel_id).or_insert_with(|| ReadState {
			id: channel_id,
			..ReadState::default()
		});
		state.mention_count = Some(state.mention_count.unwrap_or(0) + 1);
	}
}
