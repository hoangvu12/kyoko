use std::sync::Arc;
use std::time::Duration;

use net::Trust;
use parking_lot::Mutex;
use voice::video::VideoCodecs;
use voice::{AudioIo, MicCheck, VoiceConnection};

use crate::video_feed::{VideoFeed, VideoView};

const JOIN_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraIo {
	#[default]
	Devices,
	Pattern,
}

#[derive(Clone)]
pub struct VoiceSetup {
	pub audio: AudioIo,
	pub trust: Trust,
	pub join_timeout: Duration,
	pub camera: CameraIo,
	pub codecs: VideoCodecs,
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
}
