use protocol::{Permissions, Snowflake};
use rest::api::ReactionRef;

use super::Core;
use super::guild_writes::{Call, Write};

impl Core {
	pub(super) fn add_reaction(
		&mut self,
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
	) {
		let joining = self.has_reaction(channel_id, message_id, &emoji);
		if !self.channel_allows(
			channel_id,
			Permissions::READ_MESSAGE_HISTORY,
			"Read Message History",
		) {
			return;
		}
		if !joining && !self.channel_allows(channel_id, Permissions::ADD_REACTIONS, "Add Reactions") {
			return;
		}
		if burst && !self.state.read().has_nitro() {
			self.reject("Super reactions need Nitro on this account");
			return;
		}
		self.remember_reaction(&emoji);
		self.spawn_action("add reaction", move |rest| async move {
			let reaction = ReactionRef {
				channel_id,
				message_id,
				emoji: &emoji,
				burst,
			};
			rest.add_reaction(reaction).await
		});
	}

	pub(super) fn remove_reaction(
		&mut self,
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
	) {
		self.spawn_action("remove reaction", move |rest| async move {
			let reaction = ReactionRef {
				channel_id,
				message_id,
				emoji: &emoji,
				burst,
			};
			rest.remove_own_reaction(reaction).await
		});
	}

	pub(super) fn remove_user_reaction(
		&mut self,
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
		user_id: Snowflake,
	) {
		let Some(guild_id) = self.state.read().channel_guild(channel_id) else {
			self.reject("This only works in a server");
			return;
		};
		if !self.channel_allows(channel_id, Permissions::MANAGE_MESSAGES, "Manage Messages") {
			return;
		}
		let call = Call::RemoveReaction {
			channel_id,
			message_id,
			emoji,
			burst,
			user_id,
		};
		self.queue_guild_write(guild_id, Write::new("remove reaction", call));
	}

	fn has_reaction(&self, channel_id: Snowflake, message_id: Snowflake, emoji: &str) -> bool {
		let state = self.state.read();
		state
			.messages(channel_id)
			.and_then(|cache| cache.by_id.get(&message_id))
			.is_some_and(|message| {
				message
					.reactions
					.iter()
					.any(|reaction| state::reaction_name(&reaction.emoji) == emoji)
			})
	}
}
