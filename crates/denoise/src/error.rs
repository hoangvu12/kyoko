#[derive(Debug, Clone, thiserror::Error)]
pub enum DenoiseError {
	#[error("the {0} weights do not match their hash")]
	Integrity(&'static str),
	#[error("the noise model failed: {0}")]
	Model(String),
}

pub type Result<T> = std::result::Result<T, DenoiseError>;
