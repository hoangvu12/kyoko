use std::time::Duration;

use protocol::DecodeError;
use protocol::rest::ApiError;
use reqwest::StatusCode;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, RestError>;

#[derive(Debug, Error)]
pub enum RestError {
	#[error("http status {0}")]
	Http(StatusCode, Option<ApiError>),
	#[error("rate limited, retry in {retry_after:?}")]
	RateLimited { retry_after: Duration },
	#[error("network error: {0}")]
	Network(String),
	#[error(transparent)]
	Decode(DecodeError),
	#[error("unexpected redirect")]
	Redirect,
	#[error("unauthorized")]
	Unauthorized,
	#[error("forbidden")]
	Forbidden,
	#[error("missing permissions")]
	MissingPermissions,
	#[error("route suspended after repeated forbidden responses, retry in {retry_in:?}")]
	Suspended {
		retry_in: Duration,
		missing_permissions: bool,
	},
	#[error("captcha required")]
	Captcha { service: Option<String> },
	#[error("not found")]
	NotFound,
	#[error("response body too large")]
	TooLarge,
	#[error("invalid request")]
	InvalidRequest,
}

pub(crate) fn network_error(error: reqwest::Error) -> RestError {
	let reason = if error.is_timeout() {
		"request timed out"
	} else if error.is_connect() {
		"connection failed"
	} else if error.is_body() || error.is_decode() {
		"body read failed"
	} else {
		"request failed"
	};
	RestError::Network(reason.to_owned())
}
