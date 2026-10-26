use bridge::MessageEntry;
use mockcord::fixtures::showcase;
use zgx::input::InputEvent;
use zgx::scene::Rect;
use zgx::widgets::WeakUi;

use super::Link;
use crate::capture::{self, ReactionShot};

fn center(rect: Rect) -> (f32, f32) {
	(rect.x + rect.width / 2., rect.y + rect.height / 2.)
}

impl Link {
	fn reaction_message(&self) -> Option<MessageEntry> {
		self.timeline
			.borrow()
			.messages()
			.find(|entry| entry.content == showcase::REACTIONS_MESSAGE)
			.cloned()
	}

	fn hover_at(&self, rect: Option<Rect>, what: &str) {
		let Some(rect) = rect else {
			tracing::warn!(what, "nothing to hover");
			return;
		};
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			return;
		};
		let (x, y) = center(rect);
		ui.dispatch(InputEvent::PointerMove { x, y });
	}

	pub(super) async fn drive_reaction_shot(&self, shot: ReactionShot) {
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
		self.until_capture(|link| link.reaction_message().is_some() && link.timeline_view.settled())
			.await;
		zgx::timer::sleep(capture::SETTLE).await;
		let Some(entry) = self.reaction_message() else {
			tracing::warn!("the reactions message was not found");
			return;
		};
		let wanted = |emoji: &str| {
			bridge::reaction_chips(&entry.message)
				.into_iter()
				.find(|chip| !chip.burst && chip.key == emoji)
		};
		match shot {
			ReactionShot::Chips => {
				let anchor = wanted(showcase::TOOLTIP_EMOJI).and_then(|chip| {
					self.timeline_view
						.reaction_anchor(&format!("reaction-{}-{}", entry.id, chip.id()))
				});
				self.hover_at(anchor, "the reaction chip");
			}
			ReactionShot::List => {
				let chip = wanted(showcase::LISTED_EMOJI);
				self.timeline_view.open_reactions(&entry, chip.as_ref());
				self.until_capture(|link| link.timeline_view.reactions_settled())
					.await;
				zgx::timer::sleep(capture::SETTLE).await;
				self.hover_at(self.timeline_view.reaction_anchor("reactor-row-1"), "a reactor");
			}
		}
	}
}
