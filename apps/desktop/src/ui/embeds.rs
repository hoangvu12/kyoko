use std::path::PathBuf;
use std::rc::Rc;

use bridge::{ProviderPlayer, RowKey, StandInFrames};
use mockcord::fixtures::showcase;
use zgx::input::{InputEvent, PointerButton};
use zgx::widgets::WeakUi;

use super::{FRAME_POLL, FRAME_TRIES, Link};
use crate::capture;
use crate::embed_frames::WebFrames;

pub const PROFILE: &str = "embed-players";

impl Link {
	pub(super) async fn attach_embed_frames(self: Rc<Self>, profile: Option<PathBuf>) {
		let players = self.timeline_view.pictures().players();
		let Some(profile) = profile else {
			players.set_host(Box::new(StandInFrames::new(true)));
			return;
		};
		for _ in 0..FRAME_TRIES {
			if let Some(frame) = self.window.frame.get() {
				players.set_host(Box::new(WebFrames::new(frame.handle(), profile)));
				return;
			}
			zgx::timer::sleep(FRAME_POLL).await;
		}
		tracing::warn!("the native window was not found; embed players open in the browser");
	}

	fn provider_section(&self) -> Option<(String, RowKey)> {
		let timeline = self.timeline.borrow();
		let mut messages = timeline
			.messages()
			.skip_while(|entry| entry.content != showcase::marker(showcase::PROVIDER_SECTIONS[0].1));
		let posted = messages.find(|entry| {
			entry
				.message
				.embeds
				.first()
				.and_then(ProviderPlayer::of)
				.is_some()
		})?;
		let key = format!("embed-{}-0", posted.message.id);
		Some((key, RowKey::Message(posted.id)))
	}

	pub(super) async fn drive_provider_player(self: &Rc<Self>) {
		let players = self.timeline_view.pictures().players();
		self.reveal(Some(&showcase::marker(showcase::PROVIDER_SECTIONS[0].1)))
			.await;
		self.until_capture(|link| link.provider_section().is_some() && link.timeline_view.settled())
			.await;
		let Some((key, row)) = self.provider_section() else {
			tracing::warn!("the showcase providers were not found");
			return;
		};
		for _ in 0..capture::SCROLL_PASSES {
			self.timeline_view.scroll_to_end(&row);
			zgx::timer::sleep(capture::POLL).await;
		}
		zgx::timer::sleep(capture::SETTLE).await;
		let button = format!("provider-play-{key}");
		let Some(rect) = players.anchor(&button) else {
			tracing::warn!(button, "the play button was not found");
			return;
		};
		if let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) {
			let (x, y) = (rect.x + rect.width / 2., rect.y + rect.height / 2.);
			let button = PointerButton::Primary;
			ui.dispatch(InputEvent::PointerMove { x, y });
			ui.dispatch(InputEvent::PointerDown { x, y, button });
			ui.dispatch(InputEvent::PointerUp { x, y, button });
		}
		self.until_capture(|_| {
			players.live().as_deref() == Some(key.as_str()) && players.anchor(&key).is_some()
		})
		.await;
		zgx::timer::sleep(capture::SETTLE).await;
		tracing::info!(key, "the embed player is open for the capture");
	}
}
