use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use protocol::Snowflake;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::error::Result;
use crate::secrets::{SecretSlot, Secrets, TokenBackend};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
	pub user_id: Snowflake,
	pub username: String,
	pub avatar: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AccountIndex {
	current: Option<Snowflake>,
	accounts: Vec<Account>,
}

#[derive(Debug)]
pub struct AccountStore {
	index_path: PathBuf,
	tokens_dir: PathBuf,
	secrets: Arc<Secrets>,
	index: Mutex<AccountIndex>,
}

impl AccountStore {
	pub(crate) fn new(index_path: PathBuf, tokens_dir: PathBuf, secrets: Arc<Secrets>) -> Self {
		let index = load_index(&index_path);
		Self {
			index_path,
			tokens_dir,
			secrets,
			index: Mutex::new(index),
		}
	}

	pub fn list(&self) -> Vec<Account> {
		self.index.lock().accounts.clone()
	}

	pub fn current(&self) -> Option<Snowflake> {
		self.index.lock().current
	}

	pub fn save(
		&self,
		user_id: Snowflake,
		username: &str,
		avatar: Option<&str>,
		token: &str,
	) -> Result<TokenBackend> {
		let backend = self.slot(user_id).save(&self.secrets, token)?;
		let account = Account {
			user_id,
			username: username.to_owned(),
			avatar: avatar.map(str::to_owned),
		};
		let mut index = self.index.lock();
		match index.accounts.iter_mut().find(|known| known.user_id == user_id) {
			Some(known) => *known = account,
			None => index.accounts.push(account),
		}
		self.write(&index)?;
		Ok(backend)
	}

	pub fn token_for(&self, user_id: Snowflake) -> Result<Option<String>> {
		Ok(self
			.slot(user_id)
			.load(&self.secrets)?
			.map(|token| token.as_str().to_owned()))
	}

	pub fn forget_token(&self, user_id: Snowflake, token: &str) -> Result<bool> {
		self.slot(user_id).clear_if(&self.secrets, token)
	}

	pub fn remove(&self, user_id: Snowflake) -> Result<()> {
		let cleared = self.slot(user_id).clear(&self.secrets);
		let mut index = self.index.lock();
		index.accounts.retain(|account| account.user_id != user_id);
		if index.current == Some(user_id) {
			index.current = None;
		}
		self.write(&index)?;
		cleared
	}

	pub fn set_current(&self, user_id: Snowflake) -> Result<()> {
		let mut index = self.index.lock();
		if index.current == Some(user_id) || !index.accounts.iter().any(|account| account.user_id == user_id)
		{
			return Ok(());
		}
		index.current = Some(user_id);
		self.write(&index)
	}

	pub(crate) fn slots(&self) -> Vec<SecretSlot> {
		self.list()
			.into_iter()
			.map(|account| self.slot(account.user_id))
			.collect()
	}

	fn slot(&self, user_id: Snowflake) -> SecretSlot {
		SecretSlot::new(
			format!("account-{user_id}"),
			self.tokens_dir.join(format!("{user_id}.token")),
		)
	}

	fn write(&self, index: &AccountIndex) -> Result<()> {
		let bytes = serde_json::to_vec_pretty(index)?;
		write_atomic(&self.index_path, &bytes)
	}
}

fn load_index(path: &PathBuf) -> AccountIndex {
	match fs::read(path) {
		Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
			tracing::warn!(%error, "accounts file is malformed, starting empty");
			AccountIndex::default()
		}),
		Err(error) if error.kind() == ErrorKind::NotFound => AccountIndex::default(),
		Err(error) => {
			tracing::warn!(%error, "could not read the accounts file");
			AccountIndex::default()
		}
	}
}
