use thiserror::Error;

#[derive(Debug, Error)]
pub enum VideoError {
	#[error("video is not supported on this platform")]
	Unsupported,
	#[error("no h264 decoder was found")]
	NotFound,
	#[error("video frame is invalid: {0}")]
	InvalidFrame(&'static str),
	#[error("video device error: {0}")]
	Device(String),
	#[error("h264 decode error: {0}")]
	Decode(String),
}
