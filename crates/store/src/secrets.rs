use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use zeroize::Zeroizing;

use crate::atomic::write_atomic;
use crate::error::{Result, StoreError};

const KEYRING_SERVICE: &str = "kyoko";
const UNAVAILABLE: &str = "credential storage is not accessible";

pub trait Keyring: Send + Sync {
	fn get(&self, user: &str) -> Result<Option<Zeroizing<String>>>;
	fn set(&self, user: &str, secret: &str) -> Result<()>;
	fn delete(&self, user: &str) -> Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemKeyring;

impl SystemKeyring {
	fn entry(user: &str) -> Result<keyring::Entry> {
		Ok(keyring::Entry::new(KEYRING_SERVICE, user)?)
	}
}

impl Keyring for SystemKeyring {
	fn get(&self, user: &str) -> Result<Option<Zeroizing<String>>> {
		match Self::entry(user)?.get_password() {
			Ok(secret) => Ok(Some(Zeroizing::new(secret))),
			Err(keyring::Error::NoEntry) => Ok(None),
			Err(error) => Err(error.into()),
		}
	}

	fn set(&self, user: &str, secret: &str) -> Result<()> {
		Ok(Self::entry(user)?.set_password(secret)?)
	}

	fn delete(&self, user: &str) -> Result<()> {
		match Self::entry(user)?.delete_credential() {
			Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
			Err(error) => Err(error.into()),
		}
	}
}

#[derive(Default)]
pub struct MemoryKeyring {
	entries: Mutex<HashMap<String, Zeroizing<String>>>,
	failing: AtomicBool,
	failing_writes: AtomicBool,
}

impl MemoryKeyring {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn set_failing(&self, failing: bool) {
		self.failing.store(failing, Ordering::Relaxed);
	}

	pub fn set_failing_writes(&self, failing: bool) {
		self.failing_writes.store(failing, Ordering::Relaxed);
	}

	pub fn users(&self) -> Vec<String> {
		let mut users: Vec<String> = self.entries.lock().keys().cloned().collect();
		users.sort();
		users
	}

	pub fn secret(&self, user: &str) -> Option<Zeroizing<String>> {
		self.entries.lock().get(user).cloned()
	}

	fn available(&self) -> Result<()> {
		if self.failing.load(Ordering::Relaxed) {
			return Err(StoreError::Keyring(UNAVAILABLE));
		}
		Ok(())
	}

	fn writable(&self) -> Result<()> {
		self.available()?;
		if self.failing_writes.load(Ordering::Relaxed) {
			return Err(StoreError::Keyring(UNAVAILABLE));
		}
		Ok(())
	}
}

impl Keyring for MemoryKeyring {
	fn get(&self, user: &str) -> Result<Option<Zeroizing<String>>> {
		self.available()?;
		Ok(self.secret(user))
	}

	fn set(&self, user: &str, secret: &str) -> Result<()> {
		self.writable()?;
		self.entries
			.lock()
			.insert(user.to_owned(), Zeroizing::new(secret.to_owned()));
		Ok(())
	}

	fn delete(&self, user: &str) -> Result<()> {
		self.writable()?;
		self.entries.lock().remove(user);
		Ok(())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SecretHealth {
	pub plaintext: bool,
	pub keyring_error: Option<&'static str>,
	pub cache_in_memory: bool,
}

pub(crate) struct Secrets {
	keyring: Arc<dyn Keyring>,
	plaintext: AtomicBool,
	keyring_error: Mutex<Option<&'static str>>,
	cache_in_memory: AtomicBool,
}

impl std::fmt::Debug for Secrets {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Secrets")
			.field("plaintext", &self.plaintext())
			.finish_non_exhaustive()
	}
}

impl Secrets {
	pub(crate) fn new(keyring: Arc<dyn Keyring>, plaintext: bool) -> Self {
		Self {
			keyring,
			plaintext: AtomicBool::new(plaintext),
			keyring_error: Mutex::new(None),
			cache_in_memory: AtomicBool::new(false),
		}
	}

	pub(crate) fn plaintext(&self) -> bool {
		self.plaintext.load(Ordering::Relaxed)
	}

	pub(crate) fn set_plaintext(&self, plaintext: bool) {
		self.plaintext.store(plaintext, Ordering::Relaxed);
	}

	pub(crate) fn set_cache_in_memory(&self) {
		self.cache_in_memory.store(true, Ordering::Relaxed);
	}

	pub(crate) fn health(&self) -> SecretHealth {
		SecretHealth {
			plaintext: self.plaintext(),
			keyring_error: *self.keyring_error.lock(),
			cache_in_memory: self.cache_in_memory.load(Ordering::Relaxed),
		}
	}

	fn clear_error(&self) {
		*self.keyring_error.lock() = None;
	}

	fn keyring<T>(&self, call: impl FnOnce(&dyn Keyring) -> Result<T>) -> Result<T> {
		let result = call(self.keyring.as_ref());
		match &result {
			Ok(_) => {}
			Err(StoreError::Keyring(reason)) => {
				tracing::warn!(reason, "credential manager request failed");
				*self.keyring_error.lock() = Some(reason);
			}
			Err(_) => {}
		}
		result
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenBackend {
	Keyring,
	PlaintextFile,
}

#[derive(Debug)]
pub(crate) struct SecretSlot {
	user: String,
	file: PathBuf,
}

impl SecretSlot {
	pub(crate) fn new(user: impl Into<String>, file: PathBuf) -> Self {
		Self {
			user: user.into(),
			file,
		}
	}

	pub(crate) fn save(&self, secrets: &Secrets, secret: &str) -> Result<TokenBackend> {
		if secrets.plaintext() {
			self.write_file(secret)?;
			return Ok(TokenBackend::PlaintextFile);
		}
		secrets.keyring(|keyring| keyring.set(&self.user, secret))?;
		secrets.clear_error();
		remove_file(&self.file)?;
		Ok(TokenBackend::Keyring)
	}

	pub(crate) fn load(&self, secrets: &Secrets) -> Result<Option<Zeroizing<String>>> {
		if secrets.plaintext() {
			return read_file(&self.file);
		}
		secrets.keyring(|keyring| keyring.get(&self.user))
	}

	pub(crate) fn clear(&self, secrets: &Secrets) -> Result<()> {
		remove_file(&self.file)?;
		if secrets.plaintext() {
			return Ok(());
		}
		secrets.keyring(|keyring| keyring.delete(&self.user))
	}

	pub(crate) fn clear_if(&self, secrets: &Secrets, secret: &str) -> Result<bool> {
		let matches = self
			.load(secrets)?
			.is_some_and(|stored| stored.as_str() == secret);
		if matches {
			self.clear(secrets)?;
		}
		Ok(matches)
	}

	pub(crate) fn copy_to(&self, secrets: &Secrets, plaintext: bool) -> Result<bool> {
		let Some(secret) = self.load(secrets)? else {
			return Ok(false);
		};
		if plaintext {
			self.write_file(&secret)?;
		} else {
			secrets.keyring(|keyring| keyring.set(&self.user, &secret))?;
		}
		Ok(true)
	}

	fn write_file(&self, secret: &str) -> Result<()> {
		if let Some(parent) = self.file.parent() {
			fs::create_dir_all(parent)?;
		}
		write_atomic(&self.file, secret.as_bytes())
	}

	pub(crate) fn remove_from(&self, secrets: &Secrets, plaintext: bool) -> Result<()> {
		if plaintext {
			remove_file(&self.file)
		} else {
			secrets.keyring(|keyring| keyring.delete(&self.user))
		}
	}
}

fn read_file(path: &PathBuf) -> Result<Option<Zeroizing<String>>> {
	match fs::read_to_string(path) {
		Ok(secret) => Ok(Some(Zeroizing::new(secret))),
		Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
		Err(error) => Err(error.into()),
	}
}

fn remove_file(path: &PathBuf) -> Result<()> {
	match fs::remove_file(path) {
		Ok(()) => Ok(()),
		Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error.into()),
	}
}
