use protocol::{EditMessage, MessageFlags, Snowflake};

use super::Core;

impl Core {
	pub(super) fn suppress_embeds(&mut self, channel_id: Snowflake, message_id: Snowflake) {
		let flags = {
			let state = self.state.read();
			let me = state.current_user_id();
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&message_id))
				.filter(|message| me.is_some() && Some(message.author.id) == me)
				.map(|message| message.flags)
		};
		let Some(flags) = flags else {
			self.reject("You can only remove embeds from your own messages");
			return;
		};
		if flags.contains(MessageFlags::SUPPRESS_EMBEDS) {
			return;
		}
		let body = EditMessage {
			flags: Some(flags | MessageFlags::SUPPRESS_EMBEDS),
			..EditMessage::default()
		};
		self.spawn_action("Remove embeds", move |rest| async move {
			rest.edit_message(channel_id, message_id, body).await.map(drop)
		});
	}
}
