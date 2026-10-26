use bridge::{MessageEntry, RowKey};
use mockcord::fixtures::showcase;
use protocol::Snowflake;
use zgx::input::{InputEvent, PointerButton};
use zgx::scene::Rect;
use zgx::widgets::WeakUi;

use super::Link;
use crate::capture;

const SEEK_TO: f32 = 0.5;
const AWAY: (f32, f32) = (640., 4.);

impl Link {
	fn voice_section(&self) -> Option<(Snowflake, Snowflake)> {
		let timeline = self.timeline.borrow();
		let mut messages = timeline
			.messages()
			.skip_while(|entry| entry.content != showcase::marker(showcase::VOICE_SECTION));
		let voice = messages.find(|entry| is_voice(entry))?;
		let caption = messages.find(|entry| entry.content == showcase::AUDIO_CAPTION)?;
		Some((voice.message.attachments.first()?.id, caption.id))
	}

	fn click_at(&self, rect: Option<Rect>, across: f32, what: &str) {
		let Some(rect) = rect else {
			tracing::warn!(what, "nothing to click");
			return;
		};
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			return;
		};
		let (x, y) = (rect.x + rect.width * across, rect.y + rect.height / 2.);
		let button = PointerButton::Primary;
		ui.dispatch(InputEvent::PointerMove { x, y });
		ui.dispatch(InputEvent::PointerDown { x, y, button });
		ui.dispatch(InputEvent::PointerUp { x, y, button });
	}

	pub(super) async fn drive_voice_message(&self) {
		let listening = self.timeline_view.pictures().listening();
		listening.freeze();
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
		self.until_capture(|link| link.voice_section().is_some() && link.timeline_view.settled())
			.await;
		let Some((voice, caption)) = self.voice_section() else {
			tracing::warn!("the voice messages were not found");
			return;
		};
		for _ in 0..capture::SCROLL_PASSES {
			self.timeline_view.scroll_to_end(&RowKey::Message(caption));
			zgx::timer::sleep(capture::POLL).await;
		}
		zgx::timer::sleep(capture::SETTLE).await;
		self.click_at(
			listening.anchor(&format!("voice-play-{voice}")),
			0.5,
			"the play button",
		);
		self.until_capture(|_| listening.player().playing().is_some())
			.await;
		self.click_at(
			listening.anchor(&format!("voice-waveform-{voice}")),
			SEEK_TO,
			"the waveform",
		);
		if let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) {
			let (x, y) = AWAY;
			ui.dispatch(InputEvent::PointerMove { x, y });
		}
		zgx::timer::sleep(capture::SETTLE).await;
	}
}

fn is_voice(entry: &MessageEntry) -> bool {
	entry
		.message
		.attachments
		.iter()
		.any(|attachment| bridge::is_voice_message(&entry.message, attachment))
}
