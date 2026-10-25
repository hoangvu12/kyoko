mod accounts;
mod atomic;
mod database;
mod error;
mod identity;
pub mod migrations;
mod paths;
mod retention;
mod seal;
mod secrets;
mod settings;
mod token;

use std::sync::Arc;

use parking_lot::RwLock;
use protocol::Snowflake;

pub use accounts::{Account, AccountStore};
pub use atomic::write_atomic;
pub use database::{
	Database, DeletedRow, KeptDeleted, KeptLimits, KeptVersion, MediaEntry, MessageLimits, ReadState, Table,
};
pub use error::{Result, StoreError};
pub use paths::Paths;
pub use retention::{Pruned, Retention};
pub use secrets::{Keyring, MemoryKeyring, SecretHealth, SystemKeyring, TokenBackend};
pub use settings::{
	AccessibilitySettings, AppearanceSettings, ChatSettings, ClientSettings, DeveloperSettings,
	KeybindSettings, NotificationSettings, PluginSettings, Settings, StartupSettings, StorageSettings,
	StreamerModeSettings, VoiceSettings, WindowSettings,
};
pub use token::TokenStore;

use crate::seal::CacheKey;
use crate::secrets::{SecretSlot, Secrets};

const ACCOUNTS_FILE: &str = "accounts.json";
const ACCOUNT_TOKENS_DIR: &str = "account-tokens";

pub struct Store {
	pub paths: Paths,
	pub settings: RwLock<Settings>,
	pub db: Database,
	pub tokens: TokenStore,
	pub accounts: AccountStore,
	secrets: Arc<Secrets>,
	cache_key: SecretSlot,
}

impl Store {
	pub fn open(paths: Paths) -> Result<Self> {
		Self::open_with(paths, Arc::new(SystemKeyring))
	}

	pub fn open_with(paths: Paths, keyring: Arc<dyn Keyring>) -> Result<Self> {
		let settings = Settings::load(&paths);
		let secrets = Arc::new(Secrets::new(keyring, settings.storage.plaintext_token));
		let cache_key = seal::cache_key_slot(paths.cache_key_path());
		let db = match seal::load_or_create_key(&cache_key, &secrets) {
			CacheKey::Stored(key) => Database::open(&paths.database_path(), &key)?,
			CacheKey::Ephemeral(key) => {
				secrets.set_cache_in_memory();
				Database::open_in_memory(&key)?
			}
		};
		let tokens = TokenStore::new(paths.token_path(), secrets.clone());
		let accounts = AccountStore::new(
			paths.data_dir().join(ACCOUNTS_FILE),
			paths.config_dir().join(ACCOUNT_TOKENS_DIR),
			secrets.clone(),
		);
		Ok(Self {
			paths,
			settings: RwLock::new(settings),
			db,
			tokens,
			accounts,
			secrets,
			cache_key,
		})
	}

	pub fn secret_health(&self) -> SecretHealth {
		self.secrets.health()
	}

	pub fn update_settings(&self, apply: impl FnOnce(&mut Settings)) -> Result<()> {
		let mut settings = self.settings.write();
		let was_plaintext = settings.storage.plaintext_token;
		apply(&mut settings);
		let plaintext = settings.storage.plaintext_token;
		let moved = if plaintext == was_plaintext {
			Ok(())
		} else {
			self.move_secrets(plaintext)
		};
		if moved.is_err() {
			settings.storage.plaintext_token = was_plaintext;
		}
		settings.save(&self.paths)?;
		moved
	}

	pub fn prune(&self, retention: &Retention) -> Result<Pruned> {
		let mut pruned = Pruned {
			messages: self.db.prune_messages(retention.message_limits())?,
			kept: self.db.prune_kept(retention.kept_limits())?,
			..Pruned::default()
		};
		retention::prune_media(
			&self.paths.media_cache_dir(),
			retention.media_bytes,
			retention.media_max_age,
			&mut pruned,
		)?;
		Ok(pruned)
	}

	pub fn wipe_account_data(&self) -> Result<()> {
		let wiped = self.db.wipe_account_data(&identity::DEVICE_KEYS);
		retention::wipe_media(&self.paths.media_cache_dir())?;
		wiped
	}

	pub fn sign_out(&self, user_id: Option<Snowflake>) -> Result<()> {
		let token = self.tokens.clear();
		let account = user_id.map_or(Ok(()), |user_id| self.accounts.remove(user_id));
		let data = self.wipe_account_data();
		token.and(account).and(data)
	}

	fn move_secrets(&self, plaintext: bool) -> Result<()> {
		let accounts = self.accounts.slots();
		let slots: Vec<&SecretSlot> = [self.tokens.slot(), &self.cache_key]
			.into_iter()
			.chain(accounts.iter())
			.collect();
		let mut copied = Vec::new();
		for slot in &slots {
			match slot.copy_to(&self.secrets, plaintext) {
				Ok(true) => copied.push(*slot),
				Ok(false) => {}
				Err(error) => {
					for slot in copied {
						let _ = slot.remove_from(&self.secrets, plaintext);
					}
					return Err(error);
				}
			}
		}
		let was_plaintext = !plaintext;
		self.secrets.set_plaintext(plaintext);
		for slot in copied {
			if let Err(error) = slot.remove_from(&self.secrets, was_plaintext) {
				tracing::warn!(%error, "a moved secret was not removed from its old place");
			}
		}
		Ok(())
	}
}
