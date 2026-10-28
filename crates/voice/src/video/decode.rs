use std::time::Duration;

use super::{VideoError, VideoFrame};

pub struct H264Decoder {
	#[cfg(windows)]
	inner: super::media_foundation::MediaFoundationDecoder,
}

impl H264Decoder {
	#[cfg(windows)]
	pub fn new() -> Result<Self, VideoError> {
		Ok(Self {
			inner: super::media_foundation::MediaFoundationDecoder::new()?,
		})
	}

	#[cfg(not(windows))]
	pub fn new() -> Result<Self, VideoError> {
		Err(VideoError::Unsupported)
	}

	#[cfg(windows)]
	pub fn decode(&mut self, frame: &[u8], timestamp: Duration) -> Result<Option<VideoFrame>, VideoError> {
		self.inner.decode(frame, timestamp)
	}

	#[cfg(not(windows))]
	pub fn decode(&mut self, _frame: &[u8], _timestamp: Duration) -> Result<Option<VideoFrame>, VideoError> {
		Err(VideoError::Unsupported)
	}
}

#[cfg(windows)]
pub fn available() -> bool {
	super::media_foundation::decoders_present()
}

#[cfg(not(windows))]
pub fn available() -> bool {
	false
}
