use std::collections::HashMap;

use protocol::{
	Channel, ChannelUnreadUpdate, ClientState, EntryContainer, LenientVec, MessageAck, ReadState,
	ReadStateContainer, ReadStateKind, Snowflake, UserGuildSettings, UserGuildSettingsContainer,
};

use crate::changes::Changes;
use crate::state::State;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheVersions {
	pub read_state: Option<u64>,
	pub user_guild_settings: Option<u64>,
}

impl CacheVersions {
	pub fn client_state(&self) -> ClientState {
		ClientState {
			read_state_version: self.read_state,
			user_guild_settings_version: self.user_guild_settings,
			..ClientState::default()
		}
	}
}

fn merges<T>(known: Option<u64>, container: &EntryContainer<T>) -> bool {
	container.partial && known.is_some() && container.version >= known
}

fn version_of(raw: Option<i64>) -> Option<u64> {
	raw.and_then(|version| u64::try_from(version).ok())
}

fn is_channel_read_state(read_state: &ReadState) -> bool {
	read_state
		.read_state_type
		.is_none_or(|kind| kind == ReadStateKind::Channel)
}

pub(crate) struct Kept {
	read_states: HashMap<Snowflake, ReadState>,
	guild_settings: HashMap<Snowflake, UserGuildSettings>,
	global_settings: Option<UserGuildSettings>,
}

impl State {
	pub fn versions(&self) -> CacheVersions {
		self.versions
	}

	pub fn client_state(&self) -> ClientState {
		self.versions.client_state()
	}

	pub fn read_states(&self) -> impl Iterator<Item = &ReadState> {
		self.read_states.values()
	}

	pub fn guild_settings_cache(&self) -> UserGuildSettingsContainer {
		let entries: Vec<UserGuildSettings> = self
			.global_settings
			.iter()
			.chain(self.guild_settings.values())
			.cloned()
			.collect();
		EntryContainer {
			entries: LenientVec::from(entries),
			version: self.versions.user_guild_settings,
			partial: false,
		}
	}

	pub(crate) fn keep_versioned(&mut self) -> Kept {
		Kept {
			read_states: std::mem::take(&mut self.read_states),
			guild_settings: std::mem::take(&mut self.guild_settings),
			global_settings: self.global_settings.take(),
		}
	}

	pub(crate) fn apply_versioned(
		&mut self,
		read_state: &ReadStateContainer,
		user_guild_settings: &UserGuildSettingsContainer,
		kept: Kept,
	) {
		if merges(self.versions.read_state, read_state) {
			self.read_states = kept.read_states;
		}
		for entry in read_state
			.entries
			.iter()
			.filter(|entry| is_channel_read_state(entry))
		{
			self.read_states.insert(entry.id, entry.clone());
		}
		self.versions.read_state = read_state.version;
		self.ack_versions.clear();
		if merges(self.versions.user_guild_settings, user_guild_settings) {
			self.guild_settings = kept.guild_settings;
			self.global_settings = kept.global_settings;
		}
		for settings in user_guild_settings.entries.iter() {
			self.store_guild_settings(settings);
		}
		self.versions.user_guild_settings = user_guild_settings.version;
	}

	pub(crate) fn accepts_ack(&mut self, ack: &MessageAck) -> bool {
		let Some(version) = version_of(ack.version) else {
			return true;
		};
		if self
			.ack_versions
			.get(&ack.channel_id)
			.is_some_and(|known| version <= *known)
		{
			return false;
		}
		self.ack_versions.insert(ack.channel_id, version);
		if let Some(known) = &mut self.versions.read_state {
			*known = (*known).max(version);
		}
		true
	}

	pub(crate) fn accepts_guild_settings(&mut self, settings: &UserGuildSettings) -> bool {
		let Some(version) = version_of(settings.version) else {
			return true;
		};
		let stored = match settings.guild_id {
			Some(id) => self.guild_settings.get(&id),
			None => self.global_settings.as_ref(),
		};
		if stored
			.and_then(|stored| version_of(stored.version))
			.is_some_and(|known| version < known)
		{
			return false;
		}
		if let Some(known) = &mut self.versions.user_guild_settings {
			*known = (*known).max(version);
		}
		true
	}

	pub(crate) fn channel_unread_update(&mut self, update: &ChannelUnreadUpdate) -> Changes {
		let mut changes = Changes::default();
		let Some(guild) = self.guilds.get_mut(&update.guild_id) else {
			return changes;
		};
		for unread in update.channel_unread_updates.iter() {
			let Some(channel) = guild.channel_mut(unread.id) else {
				continue;
			};
			if unread.last_message_id > channel.last_message_id {
				channel.last_message_id = unread.last_message_id;
				changes.read_state = true;
			}
			if unread.last_pin_timestamp > channel.last_pin_timestamp {
				channel.last_pin_timestamp = unread.last_pin_timestamp;
			}
		}
		if changes.read_state {
			changes.channel_lists.insert(update.guild_id);
			changes.guild_list = true;
		}
		changes
	}

	pub(crate) fn add_late_private_channels(&mut self, channels: &[Channel]) -> bool {
		let mut added = false;
		for channel in channels {
			if self.private_channels.contains_key(&channel.id) {
				continue;
			}
			self.store_private_channel(channel);
			added = true;
		}
		added
	}
}
