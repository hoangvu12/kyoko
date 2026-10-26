use bridge::RowKey;
use mockcord::fixtures::showcase;
use protocol::{MessageType, PartialEmoji, Snowflake};
use zgx::input::InputEvent;
use zgx::widgets::WeakUi;

use super::Link;
use super::composer::Pane;
use crate::capture::{self, PollShot};

const AWAY: (f32, f32) = (640., 4.);

fn unicode(name: &str) -> Option<PartialEmoji> {
	Some(PartialEmoji {
		id: None,
		name: Some(name.to_owned()),
		animated: false,
	})
}

impl Link {
	fn poll_rows(&self, section: &str) -> Vec<(Snowflake, MessageType, bool)> {
		let timeline = self.timeline.borrow();
		let marker = showcase::marker(section);
		timeline
			.messages()
			.skip_while(|entry| entry.content != marker)
			.skip(1)
			.take_while(|entry| !entry.content.starts_with("-# showcase:"))
			.map(|entry| (entry.id, entry.kind, entry.message.poll.is_some()))
			.collect()
	}

	fn last_of(&self, section: &str) -> Option<Snowflake> {
		self.poll_rows(section).last().map(|(id, _, _)| *id)
	}

	pub(super) async fn drive_polls(&self, shot: PollShot) {
		if shot == PollShot::Create {
			self.drive_poll_dialog().await;
			return;
		}
		let section = match shot {
			PollShot::Ended => showcase::ENDED_POLL_SECTION,
			_ => showcase::POLLS_SECTION,
		};
		let channel = self
			.sidebar
			.sidebar()
			.channels()
			.find(|channel| channel.name == showcase::CHANNEL_NAME)
			.map(|channel| channel.id);
		let Some(channel_id) = channel else {
			tracing::warn!("the showcase channel is missing");
			return;
		};
		self.select_channel(channel_id);
		self.until_capture(|link| link.last_of(section).is_some() && link.timeline_view.settled())
			.await;
		let Some(last) = self.last_of(section) else {
			tracing::warn!(section, "the poll section was not found");
			return;
		};
		for _ in 0..capture::SCROLL_PASSES {
			self.timeline_view.scroll_to_end(&RowKey::Message(last));
			zgx::timer::sleep(capture::POLL).await;
		}
		if let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) {
			let (x, y) = AWAY;
			ui.dispatch(InputEvent::PointerMove { x, y });
		}
		zgx::timer::sleep(capture::SETTLE).await;
	}

	async fn drive_poll_dialog(&self) {
		self.until_capture(|link| link.composer_slot().is_some()).await;
		self.composer_layer.open_poll(Pane::Main);
		self.composer_layer.poll_form().fill(
			showcase::POLL_QUESTION,
			&[
				("Valorant", unicode("\u{1f3af}")),
				("Minecraft", unicode("\u{26cf}\u{fe0f}")),
				("Lethal Company", None),
				("", None),
			],
		);
		if let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) {
			let (x, y) = AWAY;
			ui.dispatch(InputEvent::PointerMove { x, y });
		}
		zgx::timer::sleep(capture::SETTLE).await;
	}
}
