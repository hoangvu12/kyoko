use std::time::Duration;

use bridge::{RowKey, VideoClip, VideoPhase};
use mockcord::fixtures::showcase;
use protocol::Snowflake;
use zgx::input::{InputEvent, PointerButton};
use zgx::scene::Rect;
use zgx::widgets::WeakUi;

use super::Link;
use crate::capture;

const SEEK_TO: f32 = 0.5;
const SOUGHT: Duration = Duration::from_millis(1500);
const NEAR: Duration = Duration::from_millis(200);
const AWAY: (f32, f32) = (640., 4.);

impl Link {
	fn video_section(&self) -> Option<(VideoClip, Snowflake)> {
		let timeline = self.timeline.borrow();
		let mut messages = timeline
			.messages()
			.skip_while(|entry| entry.content != showcase::marker(showcase::VIDEO_SECTIONS[0].1));
		let posted = messages.find(|entry| entry.content == showcase::VIDEO_CAPTION)?;
		let clip = VideoClip::attachment(&posted.message, posted.message.attachments.first()?)?;
		let after = messages.next()?;
		Some((clip, after.id))
	}

	fn press_video(&self, rect: Option<Rect>, across: f32, what: &str) {
		let Some(rect) = rect else {
			tracing::warn!(what, "nothing to press");
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

	pub(super) async fn drive_video_player(&self) {
		let watching = self.timeline_view.pictures().watching();
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
		self.until_capture(|link| link.video_section().is_some() && link.timeline_view.settled())
			.await;
		let Some((clip, after)) = self.video_section() else {
			tracing::warn!("the showcase video was not found");
			return;
		};
		for _ in 0..capture::SCROLL_PASSES {
			self.timeline_view.scroll_to_end(&RowKey::Message(after));
			zgx::timer::sleep(capture::POLL).await;
		}
		zgx::timer::sleep(capture::SETTLE).await;
		self.press_video(
			watching.anchor(&format!("video-{}", clip.key)),
			0.5,
			"the play button",
		);
		self.until_capture(|_| watching.frames_shown() > 0 && watching.player().state(&clip).is_playing())
			.await;
		watching.player().pause(&clip);
		self.until_capture(|_| watching.player().state(&clip).phase == VideoPhase::Paused)
			.await;
		zgx::timer::sleep(capture::SETTLE).await;
		self.press_video(
			watching.anchor(&format!("video-seek-{}", clip.key)),
			SEEK_TO,
			"the seek bar",
		);
		self.until_capture(|_| watching.player().state(&clip).position.abs_diff(SOUGHT) <= NEAR)
			.await;
		if let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) {
			let (x, y) = AWAY;
			ui.dispatch(InputEvent::PointerMove { x, y });
		}
		zgx::timer::sleep(capture::SETTLE).await;
		tracing::info!(
			frames = watching.frames_shown(),
			"the video player is ready for the capture"
		);
	}
}
