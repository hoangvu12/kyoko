use std::path::PathBuf;

use chacha20poly1305::{AeadInOut, KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::error::{Result, StoreError};
use crate::secrets::{SecretSlot, Secrets};

const MAGIC: &[u8; 4] = b"OXC1";
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;
const KEYRING_USER: &str = "cache-key";

pub(crate) struct Sealer {
	cipher: XChaCha20Poly1305,
}

impl std::fmt::Debug for Sealer {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("Sealer(redacted)")
	}
}

impl Sealer {
	pub(crate) fn new(key: &[u8; KEY_LEN]) -> Self {
		Self {
			cipher: XChaCha20Poly1305::new(key.into()),
		}
	}

	pub(crate) fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
		let nonce_bytes: [u8; NONCE_LEN] = rand::random();
		let mut sealed = Vec::with_capacity(MAGIC.len() + NONCE_LEN + plain.len() + TAG_LEN);
		sealed.extend_from_slice(MAGIC);
		sealed.extend_from_slice(&nonce_bytes);
		sealed.extend_from_slice(plain);
		let mut body = sealed.split_off(MAGIC.len() + NONCE_LEN);
		self.cipher
			.encrypt_in_place(&XNonce::from(nonce_bytes), MAGIC, &mut body)
			.map_err(|_| StoreError::Seal("could not encrypt"))?;
		sealed.append(&mut body);
		Ok(sealed)
	}

	pub(crate) fn open(&self, sealed: &[u8]) -> Result<Vec<u8>> {
		let header_len = MAGIC.len() + NONCE_LEN;
		if !is_sealed(sealed) || sealed.len() < header_len + TAG_LEN {
			return Err(StoreError::Seal("value is not sealed"));
		}
		let nonce =
			XNonce::try_from(&sealed[MAGIC.len()..header_len]).map_err(|_| StoreError::Seal("bad nonce"))?;
		let mut body = sealed[header_len..].to_vec();
		self.cipher
			.decrypt_in_place(&nonce, MAGIC, &mut body)
			.map_err(|_| StoreError::Seal("wrong key or corrupted value"))?;
		Ok(body)
	}
}

pub(crate) fn is_sealed(bytes: &[u8]) -> bool {
	bytes.starts_with(MAGIC)
}

pub(crate) enum CacheKey {
	Stored(Zeroizing<[u8; KEY_LEN]>),
	Ephemeral(Zeroizing<[u8; KEY_LEN]>),
}

pub(crate) fn cache_key_slot(file_path: PathBuf) -> SecretSlot {
	SecretSlot::new(KEYRING_USER, file_path)
}

pub(crate) fn load_or_create_key(slot: &SecretSlot, secrets: &Secrets) -> CacheKey {
	let stored = match slot.load(secrets) {
		Ok(stored) => stored,
		Err(error) => {
			tracing::warn!(%error, "could not read the cache key, keeping the cache in memory");
			return CacheKey::Ephemeral(random_key());
		}
	};
	if let Some(encoded) = stored {
		let mut key = Zeroizing::new([0u8; KEY_LEN]);
		if hex::decode_to_slice(encoded.trim(), key.as_mut()).is_ok() {
			return CacheKey::Stored(key);
		}
		tracing::warn!("stored cache key is malformed, creating a new one");
	}
	let key = random_key();
	match slot.save(secrets, &Zeroizing::new(hex::encode(key.as_ref()))) {
		Ok(_) => CacheKey::Stored(key),
		Err(error) => {
			tracing::warn!(%error, "could not save the cache key, keeping the cache in memory");
			CacheKey::Ephemeral(key)
		}
	}
}

fn random_key() -> Zeroizing<[u8; KEY_LEN]> {
	Zeroizing::new(rand::random::<[u8; KEY_LEN]>())
}
