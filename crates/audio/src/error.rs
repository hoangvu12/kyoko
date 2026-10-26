use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AudioError {
	#[error("decode failed: {0}")]
	Decode(String),
	#[error("unsupported audio: {0}")]
	Unsupported(String),
	#[error("the audio is longer than kyoko keeps in memory")]
	TooLong,
	#[error("audio output failed: {0}")]
	Device(String),
}

pub type Result<T> = std::result::Result<T, AudioError>;
