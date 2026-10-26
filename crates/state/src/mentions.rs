use std::sync::Arc;

use chrono::Utc;
use protocol::{ChannelType, Message, Permissions, Snowflake};

use crate::changes::Changes;
use crate::notifications::guild_muted;
use crate::permissions::compute_permissions;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnreadChannel {
	pub guild_id: Snowflake,
	pub channel_id: Snowflake,
	pub mentions: u32,
}

impl State {
	pub(crate) fn recent_mentions_loaded(&mut self, messages: &[Message]) -> Changes {
		let shared = messages
			.iter()
			.map(|message| {
				let guild_id = message
					.guild_id
					.or_else(|| self.channel_guild(message.channel_id));
				self.ingest_message_people(guild_id, message);
				Arc::new(message.clone())
			})
			.collect();
		self.lazy.mentions = Some(shared);
		Changes {
			recent_mentions: true,
			..Changes::default()
		}
	}

	pub(crate) fn remove_recent_mention(&mut self, message_id: Snowflake) -> Changes {
		let Some(mentions) = self.lazy.mentions.as_mut() else {
			return Changes::default();
		};
		let before = mentions.len();
		mentions.retain(|message| message.id != message_id);
		Changes {
			recent_mentions: mentions.len() != before,
			..Changes::default()
		}
	}

	pub fn recent_mentions(&self) -> Option<&[Arc<Message>]> {
		self.lazy.mentions.as_deref()
	}

	pub fn unread_channels(&self) -> Vec<UnreadChannel> {
		let now = Utc::now();
		let Some(me) = self.current_user_id() else {
			return Vec::new();
		};
		let mut unread = Vec::new();
		for guild in self.guilds.values() {
			let settings = self.guild_settings.get(&guild.id);
			if guild_muted(settings, now) {
				continue;
			}
			let mut channels: Vec<_> = guild
				.channels
				.values()
				.filter(|channel| channel.kind != ChannelType::GuildCategory)
				.filter(|channel| {
					compute_permissions(guild, Some(channel), me, now).contains(Permissions::VIEW_CHANNEL)
				})
				.filter_map(|channel| {
					let status = self.channel_status(channel, settings, true, now);
					status.unread.then_some((
						channel.position.unwrap_or_default(),
						UnreadChannel {
							guild_id: guild.id,
							channel_id: channel.id,
							mentions: status.mentions,
						},
					))
				})
				.collect();
			channels.sort_by_key(|(position, entry)| (*position, entry.channel_id));
			unread.extend(channels.into_iter().map(|(_, entry)| entry));
		}
		unread
	}
}
