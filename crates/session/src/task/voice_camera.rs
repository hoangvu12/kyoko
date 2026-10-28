use state::LocalEvent;
use voice::video::{TestPattern, VideoError, VideoQuality, VideoSource, open_camera};

use super::{Core, Internal};
use crate::voice_setup::CameraIo;

const VIDEO_UNAVAILABLE: &str = "Video unavailable. Windows has no H.264 video encoder here.";
const CAMERA_FAILED: &str = "Couldn't start your camera.";
const CAMERA_STOPPED: &str = "Your camera stopped.";

impl Core {
	pub(super) fn set_self_video(&mut self, on: bool) {
		self.voice.camera.wanted = on;
		if on {
			self.start_camera();
		} else {
			self.stop_camera();
		}
	}

	fn start_camera(&mut self) {
		if self.voice.camera.on || self.voice.camera.starting {
			return;
		}
		let Some(connection) = self.voice.connection.clone() else {
			self.voice.camera.wanted = false;
			return;
		};
		let setup = self.voice_shared.setup.lock().clone();
		let settings = self.store.settings.read().voice.clone();
		let quality = VideoQuality::preset(settings.video_preset);
		self.voice.camera.starting = true;
		let generation = self.voice.generation;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let opened = match setup.camera {
				CameraIo::Pattern => Ok(Box::new(TestPattern::new(quality)) as Box<dyn VideoSource>),
				CameraIo::Devices => tokio::task::spawn_blocking(move || {
					open_camera(settings.camera_device.as_deref(), quality)
				})
				.await
				.unwrap_or(Err(VideoError::Stopped)),
			};
			let result = match opened {
				Ok(source) => connection.start_camera(source, quality, setup.codecs).await,
				Err(error) => Err(error),
			};
			let _ = internal.send(Internal::CameraStarted { generation, result });
		});
	}

	pub(super) fn on_camera_started(&mut self, generation: u64, result: Result<(), VideoError>) {
		if generation != self.voice.generation {
			return;
		}
		self.voice.camera.starting = false;
		match result {
			Ok(()) => {
				self.voice.camera.on = true;
				self.apply_local(LocalEvent::CameraChanged(true));
				self.resend_voice_state();
				if !self.voice.camera.wanted {
					self.stop_camera();
				}
			}
			Err(error) => {
				tracing::warn!(%error, "camera did not start");
				self.voice.camera.wanted = false;
				self.reject(if error.unavailable() {
					VIDEO_UNAVAILABLE
				} else {
					CAMERA_FAILED
				});
			}
		}
	}

	fn stop_camera(&mut self) {
		if !self.voice.camera.on {
			return;
		}
		if let Some(connection) = &self.voice.connection {
			connection.stop_camera();
		}
		self.voice.camera.on = false;
		self.apply_local(LocalEvent::CameraChanged(false));
		self.resend_voice_state();
	}

	pub(super) fn camera_stopped(&mut self) {
		if self.voice.camera.on {
			self.voice.camera.wanted = false;
			self.stop_camera();
			self.reject(CAMERA_STOPPED);
		}
	}
}
