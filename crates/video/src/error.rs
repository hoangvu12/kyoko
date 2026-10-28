use thiserror::Error;

#[derive(Debug, Error)]
pub enum VideoError {
	#[error("video is not supported on this platform")]
	Unsupported,
	#[error("no h264 codec was found")]
	NotFound,
	#[error("video frame is invalid: {0}")]
	InvalidFrame(&'static str),
	#[error("video device error: {0}")]
	Device(String),
	#[error("h264 decode error: {0}")]
	Decode(String),
	#[error("h264 encode error: {0}")]
	Encode(String),
	#[error("the voice server assigned no video streams")]
	NoLayers,
	#[error("the camera is already on")]
	Busy,
	#[error("the camera stopped")]
	Stopped,
	#[error("the camera sent no frames")]
	Timeout,
}

impl VideoError {
	pub fn unavailable(&self) -> bool {
		matches!(self, Self::Unsupported | Self::NotFound)
	}
}
