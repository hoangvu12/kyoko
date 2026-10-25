use std::time::Duration;

use protocol::{Token, User};
use rest::{ClientIdentity, RestClient, RestConfig, RestTuning};
use url::Url;
use zeroize::Zeroizing;

use crate::error::{LoginError, Result};

pub struct TokenLogin;

pub(crate) fn clean_token(input: &str) -> &str {
	input
		.trim()
		.trim_matches(|c| matches!(c, '"' | '\'' | '`'))
		.trim()
}

pub(crate) fn plausible(token: &str) -> bool {
	!token.is_empty() && token.len() <= 512 && token.bytes().all(|byte| byte.is_ascii_graphic())
}

fn without_retries() -> RestTuning {
	RestTuning {
		max_retry_wait: Duration::ZERO,
		..RestTuning::default()
	}
}

impl TokenLogin {
	pub async fn validate(rest_base: Url, identity: &ClientIdentity, token: String) -> Result<(Token, User)> {
		let pasted = Zeroizing::new(token);
		let cleaned = clean_token(&pasted);
		if !plausible(cleaned) {
			return Err(LoginError::InvalidToken);
		}
		let client = RestClient::new(RestConfig {
			tuning: without_retries(),
			..RestConfig::new(rest_base, identity.clone())
		});
		client.set_token(Token::new(cleaned));
		let user = client.current_user().await;
		client.clear_token();
		Ok((Token::new(cleaned), user?))
	}
}
