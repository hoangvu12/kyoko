use protocol::Snowflake;

use crate::State;

impl State {
	pub fn last_channel(&self, guild_id: Snowflake) -> Option<Snowflake> {
		self.last_channels.get(&guild_id).copied()
	}

	pub fn last_channels(&self) -> Vec<(Snowflake, Snowflake)> {
		let mut pairs: Vec<(Snowflake, Snowflake)> = self
			.last_channels
			.iter()
			.filter(|(guild_id, _)| self.guilds.is_empty() || self.guilds.contains_key(*guild_id))
			.map(|(guild_id, channel_id)| (*guild_id, *channel_id))
			.collect();
		pairs.sort_unstable();
		pairs
	}

	pub(crate) fn remember_channel(&mut self, guild_id: Option<Snowflake>, channel_id: Snowflake) {
		if let Some(guild_id) = guild_id {
			self.last_channels.insert(guild_id, channel_id);
		}
	}

	pub(crate) fn restore_last_channels(&mut self, pairs: &[(Snowflake, Snowflake)]) {
		for (guild_id, channel_id) in pairs {
			self.last_channels.entry(*guild_id).or_insert(*channel_id);
		}
	}
}
