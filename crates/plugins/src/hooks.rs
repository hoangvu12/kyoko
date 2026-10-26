use anyhow::Result;
use protocol::{DispatchEvent, Message, Snowflake};
use session::{
	DisplayHooks, EmojiRankRequest, Formatting, KeptMessage, MessageLook, Person, RelativeTime, Rewrite,
	SessionHooks, ShownName,
};

use crate::calls::{Call, MessageView, PluginEvent, RENDER_MESSAGE};
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
		let mut view = MessageView::of(message);
		view.guild_id = view.guild_id.or_else(|| self.guild_of(message.channel_id));
		Ok(self.allow(Hook::Notify, Call::AllowNotification(view)))
	}

	fn ranks_emoji(&self) -> bool {
		self.ranks()
	}

	fn rank_emoji(&self, request: &EmojiRankRequest) -> Result<Option<Vec<String>>> {
		Ok(self.rank(request))
	}

	fn on_dispatch(&self, event: &DispatchEvent) -> Result<()> {
		if let Some(event) = PluginEvent::of(event) {
			self.deliver(event);
		}
		Ok(())
	}
}

impl DisplayHooks for PluginHost {
	fn formatters(&self, formatting: Formatting) -> Vec<String> {
		match formatting {
			Formatting::Names => PluginHost::formatters(self, Hook::DisplayName, "format_name"),
			Formatting::RelativeTimes => PluginHost::formatters(self, Hook::Timestamp, "format_relative"),
			Formatting::Messages => PluginHost::formatters(self, Hook::MessageRender, RENDER_MESSAGE),
		}
	}

	fn format_name(&self, person: &Person) -> Option<ShownName> {
		self.name_of(person)
	}

	fn format_relative(&self, time: &RelativeTime) -> Option<String> {
		self.relative_of(time)
	}

	fn render_message(&self, message: &KeptMessage) -> Option<MessageLook> {
		self.look_of(message)
	}
}
