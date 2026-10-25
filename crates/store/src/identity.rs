use crate::error::Result;
use crate::{Database, Store};

const INSTALLATION_ID_KEY: &str = "client.installation_id";
const COOKIE_JAR_KEY: &str = "client.cookie_jar";
const LAYOUT_KEY: &str = "store.layout";
const PER_ACCOUNT: &[u8] = b"per-account";
pub(crate) const DEVICE_KEYS: [&str; 3] = [INSTALLATION_ID_KEY, COOKIE_JAR_KEY, LAYOUT_KEY];

pub(crate) fn settle_layout(device: &Database) -> Result<()> {
	if device.kv_get(LAYOUT_KEY)?.as_deref() == Some(PER_ACCOUNT) {
		return Ok(());
	}
	device.wipe_account_data(&DEVICE_KEYS)?;
	device.kv_set(LAYOUT_KEY, PER_ACCOUNT)
}

impl Store {
	pub fn installation_id(&self) -> Option<String> {
		self.read_kv(INSTALLATION_ID_KEY)
			.and_then(|bytes| String::from_utf8(bytes).ok())
	}

	pub fn save_installation_id(&self, id: &str) -> Result<()> {
		self.device.kv_set(INSTALLATION_ID_KEY, id.as_bytes())
	}

	pub fn cookie_jar(&self) -> Option<Vec<u8>> {
		self.read_kv(COOKIE_JAR_KEY)
	}

	pub fn save_cookie_jar(&self, jar: &[u8]) -> Result<()> {
		self.device.kv_set(COOKIE_JAR_KEY, jar)
	}

	fn read_kv(&self, key: &str) -> Option<Vec<u8>> {
		self.device.kv_get(key).unwrap_or_else(|error| {
			tracing::warn!(%error, key, "stored client identity is unreadable");
			None
		})
	}
}
