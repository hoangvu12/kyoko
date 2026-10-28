use state::{LocalEvent, VideoKind};
use voice::VoiceConnection;

use super::Core;
use crate::video_feed::VideoView;

impl Core {
	pub(super) fn set_video_views(&mut self, views: Vec<VideoView>) {
		self.voice_shared.feed.set_views(&views);
		*self.voice_shared.views.lock() = views;
		let connection = self.voice_shared.connection.lock().clone();
		if let Some(connection) = connection {
			self.apply_video_wants(&connection);
		}
	}

	pub(super) fn attach_video(&self, connection: &VoiceConnection) {
		connection.set_video_sink(Some(self.voice_shared.feed.sink()));
		self.apply_video_wants(connection);
	}

	pub(super) fn detach_video(&self) {
		self.voice_shared.feed.clear();
	}

	pub(super) fn remote_video(&mut self, user_id: protocol::Snowflake, active: bool) {
		self.apply_local(LocalEvent::RemoteVideo {
			user_id,
			kind: VideoKind::Camera,
			active,
		});
	}

	fn apply_video_wants(&self, connection: &VoiceConnection) {
		let wants = self
			.voice_shared
			.views
			.lock()
			.iter()
			.map(VideoView::want)
			.collect();
		connection.set_video_wants(wants);
	}
}
