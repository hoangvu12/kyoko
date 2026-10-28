use std::time::Duration;

use super::VideoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderConfig {
	pub width: u32,
	pub height: u32,
	pub fps: u32,
	pub bitrate: u32,
	pub keyframe_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
	pub data: Vec<u8>,
	pub keyframe: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCodecs {
	#[default]
	System,
	Missing,
}

impl VideoCodecs {
	pub fn encoder(self, config: EncoderConfig) -> Result<H264Encoder, VideoError> {
		match self {
			Self::System => H264Encoder::new(config),
			Self::Missing => Err(VideoError::NotFound),
		}
	}
}

pub struct H264Encoder {
	#[cfg(windows)]
	inner: super::media_foundation_encoder::MediaFoundationEncoder,
	config: EncoderConfig,
	keyframe_requested: bool,
}

impl H264Encoder {
	#[cfg(windows)]
	pub fn new(config: EncoderConfig) -> Result<Self, VideoError> {
		check(&config)?;
		Ok(Self {
			inner: super::media_foundation_encoder::MediaFoundationEncoder::new(config)?,
			config,
			keyframe_requested: true,
		})
	}

	#[cfg(not(windows))]
	pub fn new(config: EncoderConfig) -> Result<Self, VideoError> {
		check(&config)?;
		Err(VideoError::Unsupported)
	}

	pub fn config(&self) -> EncoderConfig {
		self.config
	}

	pub fn request_keyframe(&mut self) {
		self.keyframe_requested = true;
	}

	#[cfg(windows)]
	pub fn encode(&mut self, nv12: &[u8], timestamp: Duration) -> Result<Vec<EncodedFrame>, VideoError> {
		let keyframe = std::mem::take(&mut self.keyframe_requested);
		self.inner.encode(nv12, timestamp, keyframe)
	}

	#[cfg(not(windows))]
	pub fn encode(&mut self, _nv12: &[u8], _timestamp: Duration) -> Result<Vec<EncodedFrame>, VideoError> {
		Err(VideoError::Unsupported)
	}
}

fn check(config: &EncoderConfig) -> Result<(), VideoError> {
	let even = |value: u32| value >= 2 && value.is_multiple_of(2);
	if even(config.width) && even(config.height) {
		Ok(())
	} else {
		Err(VideoError::InvalidFrame("encoder dimensions must be even"))
	}
}

#[cfg(windows)]
pub fn encoders_present() -> bool {
	super::media_foundation_encoder::encoders_present()
}

#[cfg(not(windows))]
pub fn encoders_present() -> bool {
	false
}

pub fn available() -> bool {
	super::decode::available() && encoders_present()
}
