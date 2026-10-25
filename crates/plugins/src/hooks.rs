use anyhow::Result;
use protocol::{DispatchEvent, Message, Snowflake};
use session::{Rewrite, SessionHooks};

use crate::calls::{Call, MessageView, PluginEvent};
use crate::host::PluginHost;
use crate::permission::Hook;

impl SessionHooks for PluginHost {
	fn name(&self) -> &str {
		"plugins"
	}

	fn before_send(&self, channel_id: Snowflake, content: &str) -> Result<Option<Rewrite>> {
		let guild_id = self.guild_of(channel_id);
		let rewritten = self.rewrite(
			Hook::Send,
			|content| Call::BeforeSend {
				channel_id,
				guild_id,
				content: content.to_owned(),
			},
			content,
		);
		Ok(rewritten.map(|(content, by)| Rewrite::by(content, by)))
	}

	fn before_edit(
		&self,
		channel_id: Snowflake,
		message_id: Snowflake,
		content: &str,
	) -> Result<Option<Rewrite>> {
		let guild_id = self.guild_of(channel_id);
		let rewritten = self.rewrite(
			Hook::Edit,
			|content| Call::BeforeEdit {
				channel_id,
				guild_id,
				message_id,
				content: content.to_owned(),
			},
			content,
		);
		Ok(rewritten.map(|(content, by)| Rewrite::by(content, by)))
	}

	fn allow_typing(&self, channel_id: Snowflake) -> Result<bool> {
		let guild_id = self.guild_of(channel_id);
		Ok(self.allow(Hook::Typing, Call::AllowTyping { channel_id, guild_id }))
	}

	fn allow_notification(&self, message: &Message) -> Result<bool> {
		Ok(self.allow(Hook::Notify, Call::AllowNotification(MessageView::of(message))))
	}

	fn on_dispatch(&self, event: &DispatchEvent) -> Result<()> {
		if let Some(event) = PluginEvent::of(event) {
			self.deliver(event);
		}
		Ok(())
	}
}
