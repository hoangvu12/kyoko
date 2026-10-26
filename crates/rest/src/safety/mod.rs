mod breaker;
mod pacing;
mod releases;
mod stop;

use protocol::rest::{ApiError, CaptchaChallenge};
use reqwest::StatusCode;

use crate::error::RestError;
use crate::tuning::RestTuning;

pub(crate) use breaker::{Admission, Breakers, Outcome};
pub(crate) use pacing::{Pace, Pacer};
pub use releases::{Release, Releases};
pub(crate) use stop::Stop;

const MISSING_PERMISSIONS: i64 = 50013;

pub(crate) struct Safety {
	pub stop: Stop,
	pub pacer: Pacer,
	pub releases: Releases,
	pub breakers: Breakers,
}

impl Safety {
	pub fn new(tuning: &RestTuning) -> Self {
		Self {
			stop: Stop::new(),
			pacer: Pacer::new(tuning),
			releases: Releases::default(),
			breakers: Breakers::new(tuning),
		}
	}
}

fn api_code(body: &[u8]) -> Option<i64> {
	serde_json::from_slice::<ApiError>(body)
		.ok()
		.map(|error| error.code)
}

fn missing_permissions(body: &[u8]) -> bool {
	api_code(body) == Some(MISSING_PERMISSIONS)
}

pub(crate) fn outcome(status: StatusCode, body: &[u8]) -> Outcome {
	if status == StatusCode::FORBIDDEN {
		Outcome::Forbidden {
			missing_permissions: missing_permissions(body),
		}
	} else if status.is_success() {
		Outcome::Allowed
	} else {
		Outcome::Inconclusive
	}
}

pub(crate) fn captcha(body: &[u8]) -> Option<RestError> {
	let challenge = serde_json::from_slice::<CaptchaChallenge>(body).ok()?;
	challenge.is_challenge().then(|| RestError::Captcha {
		service: challenge.service().map(str::to_owned),
	})
}

pub(crate) fn forbidden(body: &[u8]) -> RestError {
	if missing_permissions(body) {
		RestError::MissingPermissions
	} else {
		RestError::Forbidden
	}
}
