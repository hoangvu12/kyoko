use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use net::Trust;
use parking_lot::Mutex;
use voice::video::{TestPattern, VideoCodecs, VideoError, VideoFrame, VideoQuality, VideoSource};
use voice::{AudioIo, MicCheck, SoundSource, SoundTarget, ToneSource, VoiceConnection};

use crate::video_feed::{VideoFeed, VideoView};

const JOIN_TIMEOUT: Duration = Duration::from_secs(15);
pub const FAKE_SOUND_HZ: f32 = 440.0;
pub const FAKE_SOUND_LEVEL: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraIo {
	#[default]
	Devices,
	Pattern,
}

#[derive(Clone, Default)]
pub struct FakeCapture {
	minimised: Arc<AtomicBool>,
	sounds: Arc<Mutex<Vec<SoundTarget>>>,
}

impl FakeCapture {
	pub fn set_minimised(&self, minimised: bool) {
		self.minimised.store(minimised, Ordering::Release);
	}

	pub fn sound_targets(&self) -> Vec<SoundTarget> {
		self.sounds.lock().clone()
	}

	pub(crate) fn screen(&self, quality: VideoQuality) -> Box<dyn VideoSource> {
		Box::new(FakeScreen {
			pattern: TestPattern::new(quality),
			minimised: self.minimised.clone(),
		})
	}

	pub(crate) fn sound(&self, target: SoundTarget) -> Box<dyn SoundSource> {
		self.sounds.lock().push(target);
		Box::new(ToneSource::new(FAKE_SOUND_HZ, FAKE_SOUND_LEVEL))
	}
}

struct FakeScreen {
	pattern: TestPattern,
	minimised: Arc<AtomicBool>,
}

impl VideoSource for FakeScreen {
	fn next_frame(&mut self) -> Result<VideoFrame, VideoError> {
		self.pattern.next_frame()
	}

	fn size(&self) -> (u32, u32) {
		self.pattern.size()
	}

	fn hidden(&self) -> bool {
		self.minimised.load(Ordering::Acquire)
	}
}

#[derive(Clone)]
pub struct VoiceSetup {
	pub audio: AudioIo,
	pub trust: Trust,
	pub join_timeout: Duration,
	pub camera: CameraIo,
	pub codecs: VideoCodecs,
	pub capture: FakeCapture,
}

impl Default for VoiceSetup {
	fn default() -> Self {
		Self {
			audio: AudioIo::Devices {
				input: None,
				output: None,
			},
			trust: Trust::default(),
			join_timeout: JOIN_TIMEOUT,
			camera: CameraIo::Devices,
			codecs: VideoCodecs::System,
			capture: FakeCapture::default(),
		}
	}
}

#[derive(Default)]
pub(crate) struct VoiceShared {
	pub(crate) setup: Mutex<VoiceSetup>,
	pub(crate) connection: Mutex<Option<Arc<VoiceConnection>>>,
	pub(crate) mic_check: Mutex<Option<Arc<MicCheck>>>,
	pub(crate) feed: VideoFeed,
	pub(crate) views: Mutex<Vec<VideoView>>,
	pub(crate) stream_connection: Mutex<Option<Arc<VoiceConnection>>>,
	pub(crate) stream_feed: VideoFeed,
	pub(crate) stream_views: Mutex<Vec<VideoView>>,
	pub(crate) share_connection: Mutex<Option<Arc<VoiceConnection>>>,
}
