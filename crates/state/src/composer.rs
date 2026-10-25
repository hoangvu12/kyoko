use std::sync::Arc;

use protocol::{ApplicationCommandIndex, PremiumTier, PremiumType, Snowflake};

use crate::changes::Changes;
use crate::state::State;

const MIB: u64 = 1024 * 1024;
pub const MAX_ATTACHMENTS: usize = 10;

impl State {
	pub fn upload_limit(&self, channel_id: Snowflake) -> u64 {
		let personal = match self.current_user.as_ref().and_then(|user| user.premium_type) {
			Some(PremiumType::Nitro) => 500 * MIB,
			Some(PremiumType::NitroClassic | PremiumType::NitroBasic) => 50 * MIB,
			_ => 10 * MIB,
		};
		let boosted = self
			.channel_guild(channel_id)
			.and_then(|guild_id| self.guilds.get(&guild_id))
			.and_then(|guild| guild.premium_tier)
			.map_or(0, |tier| match tier {
				PremiumTier::Tier2 => 50 * MIB,
				PremiumTier::Tier3 => 100 * MIB,
				_ => 0,
			});
		personal.max(boosted)
	}

	pub fn message_limit(&self) -> usize {
		match self.current_user.as_ref().and_then(|user| user.premium_type) {
			Some(PremiumType::Nitro) => 4000,
			_ => 2000,
		}
	}

	pub fn command_scope(&self, channel_id: Snowflake) -> Snowflake {
		self.channel_guild(channel_id).unwrap_or(channel_id)
	}

	pub fn command_index(&self, channel_id: Snowflake) -> Option<&Arc<ApplicationCommandIndex>> {
		self.commands.get(&self.command_scope(channel_id))
	}

	pub(crate) fn command_index_loaded(
		&mut self,
		scope: Snowflake,
		index: &Arc<ApplicationCommandIndex>,
		changes: &mut Changes,
	) {
		self.commands.insert(scope, Arc::clone(index));
		changes.commands = true;
	}
}
