use protocol::Snowflake;

use super::Link;

impl Link {
	pub(crate) fn visit_guild(&self, guild_id: Snowflake) {
		self.select_guild(guild_id);
		self.land(guild_id);
	}

	pub(super) fn follow_selection(&self, before: Option<Snowflake>) {
		let after = self.sidebar.sidebar().selected();
		if let Some(guild_id) = after.filter(|_| after != before) {
			self.land(guild_id);
		}
	}

	fn land(&self, guild_id: Snowflake) {
		if self.sidebar.sidebar().selected() != Some(guild_id) || self.showing_channel() {
			return;
		}
		let landing = self
			.state()
			.and_then(|shared| bridge::landing_channel(&shared.read(), guild_id));
		if let Some(landing) = landing {
			self.select_channel(landing.channel_id);
		}
	}

	fn showing_channel(&self) -> bool {
		self.timeline.borrow().open().is_some()
			|| self.forum_open().is_some()
			|| self.sidebar.sidebar().hidden_open().is_some()
	}
}
