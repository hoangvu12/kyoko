use crate::Store;
use crate::error::Result;

const INSTALLATION_ID_KEY: &str = "client.installation_id";
const COOKIE_JAR_KEY: &str = "client.cookie_jar";
pub(crate) const DEVICE_KEYS: [&str; 2] = [INSTALLATION_ID_KEY, COOKIE_JAR_KEY];

impl Store {
	pub fn installation_id(&self) -> Option<String> {
		self.read_kv(INSTALLATION_ID_KEY)
			.and_then(|bytes| String::from_utf8(bytes).ok())
	}

	pub fn save_installation_id(&self, id: &str) -> Result<()> {
		self.db.kv_set(INSTALLATION_ID_KEY, id.as_bytes())
	}

	pub fn cookie_jar(&self) -> Option<Vec<u8>> {
		self.read_kv(COOKIE_JAR_KEY)
	}

	pub fn save_cookie_jar(&self, jar: &[u8]) -> Result<()> {
		self.db.kv_set(COOKIE_JAR_KEY, jar)
	}

	fn read_kv(&self, key: &str) -> Option<Vec<u8>> {
		self.db.kv_get(key).unwrap_or_else(|error| {
			tracing::warn!(%error, key, "stored client identity is unreadable");
			None
		})
	}
}
