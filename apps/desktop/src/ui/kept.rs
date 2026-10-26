use std::rc::Rc;

use plugins::Slot;
use protocol::Snowflake;
use session::Command;

use super::Link;
use crate::capture;
use crate::plugin_menu::{AccessoryClick, PluginMenu};

impl Link {
	pub(super) fn install_accessories(link: &Rc<Link>, menu: &PluginMenu) {
		let clicked = Rc::downgrade(link);
		menu.on_accessory(move |click| {
			if let Some(link) = clicked.upgrade() {
				link.click_accessory(click);
			}
		});
	}

	fn click_accessory(&self, click: AccessoryClick) {
		if !self.plugins.take_activation() {
			tracing::warn!(plugin = %click.plugin, "a message accessory without a click was refused");
			return;
		}
		let context = vec![
			("source".to_owned(), "slot".to_owned()),
			("slot".to_owned(), Slot::MessageAccessory.name().to_owned()),
			("channel_id".to_owned(), click.channel_id.to_string()),
			("message_id".to_owned(), click.message_id.to_string()),
		];
		let host = self.plugins.host().clone();
		self.plugins.runtime().spawn_blocking(move || {
			if let Err(error) = host.gesture(&click.plugin, &click.key, &context) {
				tracing::info!(plugin = %click.plugin, %error, "a message accessory click failed");
			}
		});
	}

	fn own_message(&self, channel_id: Snowflake, content: &str) -> Option<Snowflake> {
		let shared = self.state()?;
		let state = shared.read();
		let mine = state.current_user_id()?;
		let cache = state.messages(channel_id)?;
		if !cache.pending.is_empty() {
			return None;
		}
		cache
			.ordered
			.iter()
			.rev()
			.find(|message| message.author.id == mine && message.content == content)
			.map(|message| message.id)
	}

	async fn until_own_message(&self, channel_id: Snowflake, content: &str) -> Option<Snowflake> {
		self.until_capture(|link| link.own_message(channel_id, content).is_some())
			.await;
		self.own_message(channel_id, content)
	}

	fn decorated(&self, message_id: Snowflake) -> bool {
		self.display.pending() == 0
			&& self
				.timeline
				.borrow()
				.messages()
				.any(|entry| entry.id == message_id && entry.look.is_some())
	}

	pub(super) async fn drive_kept(&self, screen: capture::Screen) {
		self.until_capture(|link| link.plugins.host().shows_deleted())
			.await;
		let Some(channel_id) = self.timeline.borrow().open().map(|open| open.channel_id) else {
			tracing::warn!("no open channel to keep a message in");
			return;
		};
		let first = match screen {
			capture::Screen::KeptDeleted => capture::KEPT_DELETED,
			_ => capture::KEPT_EDITS[0],
		};
		self.send(Command::SendMessage {
			channel_id,
			content: first.to_owned(),
			reply_to: None,
			ping_reply: true,
			files: Vec::new(),
		});
		let Some(message_id) = self.until_own_message(channel_id, first).await else {
			tracing::warn!("the capture message did not arrive");
			return;
		};
		if screen == capture::Screen::KeptEdits {
			for content in &capture::KEPT_EDITS[1..] {
				self.send(Command::EditMessage {
					channel_id,
					message_id,
					content: (*content).to_owned(),
				});
				self.until_own_message(channel_id, content).await;
			}
		}
		self.until_capture(|link| link.decorated(message_id)).await;
	}
}
