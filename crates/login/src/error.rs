use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, LoginError>;

#[derive(Debug, Error)]
pub enum LoginError {
	#[error("the token was rejected")]
	InvalidToken,
	#[error("network error: {0}")]
	Network(String),
	#[error("discord refused the request for now")]
	Refused,
	#[error("discord asked for a captcha")]
	Captcha,
	#[error("unexpected response: {0}")]
	Protocol(String),
	#[error("login cancelled")]
	Cancelled,
	#[error("login session expired")]
	Expired,
	#[error("login helper failed: {0}")]
	Helper(String),
	#[error("storage error: {0}")]
	Storage(String),
	#[error("io error: {0}")]
	Io(#[from] io::Error),
}

impl From<rest::RestError> for LoginError {
	fn from(error: rest::RestError) -> Self {
		match error {
			rest::RestError::Unauthorized => Self::InvalidToken,
			rest::RestError::Network(reason) => Self::Network(reason),
			rest::RestError::RateLimited { .. }
			| rest::RestError::Captcha { .. }
			| rest::RestError::Suspended { .. } => Self::Refused,
			other => Self::Protocol(other.to_string()),
		}
	}
}

impl From<store::StoreError> for LoginError {
	fn from(error: store::StoreError) -> Self {
		match error {
			store::StoreError::Io(error) => Self::Io(error),
			other => Self::Storage(other.to_string()),
		}
	}
}
