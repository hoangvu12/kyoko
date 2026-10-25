use std::time::Duration;

use protocol::fingerprint::{API_VERSION, WEB_ORIGIN};
use url::Url;

use crate::identity::ClientIdentity;

#[derive(Debug, Clone)]
pub struct RestConfig {
	pub base_url: Url,
	pub identity: ClientIdentity,
	pub timeout: Duration,
	pub max_body: usize,
}

impl RestConfig {
	pub fn new(base_url: Url, identity: ClientIdentity) -> Self {
		Self {
			base_url,
			identity,
			timeout: Duration::from_secs(30),
			max_body: 16 * 1024 * 1024,
		}
	}

	pub fn default_base_url() -> Url {
		Url::parse(&format!("{WEB_ORIGIN}/api/v{API_VERSION}")).expect("default base url is valid")
	}
}
