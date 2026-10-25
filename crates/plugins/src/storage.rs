use std::path::PathBuf;

use serde_json::{Map, Value as Json};

const KEY_LIMIT: usize = 128;

pub(crate) struct Storage {
	path: Option<PathBuf>,
	values: Map<String, Json>,
	bytes: usize,
	keys: usize,
	dirty: bool,
}

impl Storage {
	pub(crate) fn open(path: Option<PathBuf>, bytes: usize, keys: usize) -> Storage {
		let values = path
			.as_ref()
			.and_then(|path| std::fs::read(path).ok())
			.and_then(|bytes| serde_json::from_slice::<Json>(&bytes).ok())
			.and_then(|json| match json {
				Json::Object(map) => Some(map),
				_ => None,
			})
			.unwrap_or_default();
		Storage {
			path,
			values,
			bytes,
			keys,
			dirty: false,
		}
	}

	pub(crate) fn get(&self, key: &str) -> Option<&Json> {
		self.values.get(key)
	}

	pub(crate) fn keys(&self) -> Vec<String> {
		self.values.keys().cloned().collect()
	}

	pub(crate) fn set(&mut self, key: &str, value: Json) -> Result<(), String> {
		if key.is_empty() || key.len() > KEY_LIMIT {
			return Err(format!("storage keys are 1 to {KEY_LIMIT} bytes long"));
		}
		if value.is_null() {
			self.delete(key);
			return Ok(());
		}
		if !self.values.contains_key(key) && self.values.len() >= self.keys {
			return Err(format!("storage holds at most {} keys", self.keys));
		}
		let previous = self.values.insert(key.to_owned(), value);
		if self.size() > self.bytes {
			match previous {
				Some(previous) => self.values.insert(key.to_owned(), previous),
				None => self.values.remove(key),
			};
			return Err(format!("storage holds at most {} bytes", self.bytes));
		}
		self.dirty = true;
		Ok(())
	}

	pub(crate) fn delete(&mut self, key: &str) {
		if self.values.remove(key).is_some() {
			self.dirty = true;
		}
	}

	pub(crate) fn flush(&mut self) {
		if !self.dirty {
			return;
		}
		self.dirty = false;
		let Some(path) = &self.path else {
			return;
		};
		if let Some(parent) = path.parent() {
			let _ = std::fs::create_dir_all(parent);
		}
		let bytes = serde_json::to_vec(&self.values).unwrap_or_default();
		if let Err(error) = store::write_atomic(path, &bytes) {
			tracing::warn!(%error, "plugin storage was not saved");
		}
	}

	fn size(&self) -> usize {
		serde_json::to_vec(&self.values).map_or(usize::MAX, |bytes| bytes.len())
	}
}
