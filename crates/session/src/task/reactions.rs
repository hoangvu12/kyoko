use protocol::{PartialEmoji, Permissions, Snowflake};

use super::Core;

fn reaction_name(emoji: &PartialEmoji) -> String {
	match (&emoji.id, &emoji.name) {
		(Some(id), name) => format!("{}:{id}", name.as_deref().unwrap_or("emoji")),
		(None, name) => name.clone().unwrap_or_default(),
	}
}

impl Core {
	pub(super) fn add_reaction(&mut self, channel_id: Snowflake, message_id: Snowflake, emoji: String) {
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
		self.remember_reaction(&emoji);
		self.spawn_action("add reaction", move |rest| async move {
			rest.add_reaction(channel_id, message_id, &emoji).await
		});
	}

	pub(super) fn remove_reaction(&mut self, channel_id: Snowflake, message_id: Snowflake, emoji: String) {
		self.spawn_action("remove reaction", move |rest| async move {
			rest.remove_own_reaction(channel_id, message_id, &emoji).await
		});
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
					.any(|reaction| reaction_name(&reaction.emoji) == emoji)
			})
	}
}
