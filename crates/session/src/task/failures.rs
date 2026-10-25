use rest::RestError;
use state::SendFailure;

pub(super) fn send_failure(error: &RestError) -> SendFailure {
	match error {
		RestError::RateLimited { retry_after } => SendFailure::RateLimited {
			retry_after: *retry_after,
		},
		RestError::Forbidden => SendFailure::Forbidden,
		RestError::MissingPermissions => SendFailure::MissingPermissions,
		RestError::Suspended {
			missing_permissions: true,
			..
		} => SendFailure::MissingPermissions,
		RestError::Suspended { .. } => SendFailure::Forbidden,
		RestError::Captcha { service } => SendFailure::Captcha {
			service: service.clone(),
		},
		RestError::Unauthorized => SendFailure::Unauthorized,
		RestError::Network(_) => SendFailure::Network,
		RestError::Http(status, _) => SendFailure::Other(Some(status.as_u16())),
		RestError::NotFound => SendFailure::Other(Some(404)),
		RestError::Decode(_) | RestError::Redirect | RestError::TooLarge | RestError::InvalidRequest => {
			SendFailure::Other(None)
		}
	}
}
