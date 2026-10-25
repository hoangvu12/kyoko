use std::path::PathBuf;
use std::sync::Arc;

use crate::error::Result;
use crate::secrets::{SecretSlot, Secrets, TokenBackend};

const KEYRING_USER: &str = "session-token";

#[derive(Debug)]
pub struct TokenStore {
	slot: SecretSlot,
	secrets: Arc<Secrets>,
}

impl TokenStore {
	pub(crate) fn new(file_path: PathBuf, secrets: Arc<Secrets>) -> Self {
		Self {
			slot: SecretSlot::new(KEYRING_USER, file_path),
			secrets,
		}
	}

	pub fn save(&self, token: &str) -> Result<TokenBackend> {
		self.slot.save(&self.secrets, token)
	}

	pub fn load(&self) -> Result<Option<String>> {
		Ok(self
			.slot
			.load(&self.secrets)?
			.map(|token| token.as_str().to_owned()))
	}

	pub fn clear(&self) -> Result<()> {
		self.slot.clear(&self.secrets)
	}

	pub fn forget(&self, token: &str) -> Result<bool> {
		self.slot.clear_if(&self.secrets, token)
	}

	pub(crate) fn slot(&self) -> &SecretSlot {
		&self.slot
	}
}
