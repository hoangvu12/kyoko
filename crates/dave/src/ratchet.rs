use std::collections::HashMap;

use btls::hash::MessageDigest;
use btls::hmac::Hmac;
use zeroize::{Zeroize, Zeroizing};

pub const KEY_BYTES: usize = 16;
const SECRET_BYTES: usize = 32;
const LABEL_PREFIX: &[u8] = b"MLS 1.0 ";
const GENERATION_CONTEXT_BYTES: u8 = 4;
const FIRST_BLOCK_COUNTER: u8 = 1;

pub type Key = [u8; KEY_BYTES];

pub struct KeyRatchet {
	base: Zeroizing<Key>,
	next_secret: Zeroizing<[u8; SECRET_BYTES]>,
	secret_len: usize,
	next_generation: u32,
	keys: HashMap<u32, Key>,
}

impl KeyRatchet {
	pub fn new(base: Key) -> Self {
		let mut next_secret = Zeroizing::new([0; SECRET_BYTES]);
		next_secret[..KEY_BYTES].copy_from_slice(&base);
		Self {
			base: Zeroizing::new(base),
			next_secret,
			secret_len: KEY_BYTES,
			next_generation: 0,
			keys: HashMap::new(),
		}
	}

	pub fn same_domain(&self, other: &Self) -> bool {
		*self.base == *other.base
	}

	pub fn key(&mut self, generation: u32) -> Option<Key> {
		if let Some(key) = self.keys.get(&generation) {
			return Some(*key);
		}
		if self.next_generation > generation {
			return None;
		}
		while self.next_generation <= generation {
			self.advance()?;
		}
		self.keys.get(&generation).copied()
	}

	pub fn erase(&mut self, generation: u32) {
		if let Some(mut key) = self.keys.remove(&generation) {
			key.zeroize();
		}
	}

	fn advance(&mut self) -> Option<()> {
		let generation = self.next_generation;
		let secret = &self.next_secret[..self.secret_len];
		let mut key = [0; KEY_BYTES];
		key.copy_from_slice(&derive(secret, b"key", generation)?[..KEY_BYTES]);
		let next = derive(secret, b"secret", generation)?;
		self.next_secret.copy_from_slice(&next[..SECRET_BYTES]);
		self.secret_len = SECRET_BYTES;
		self.next_generation = generation.wrapping_add(1);
		self.keys.insert(generation, key);
		Some(())
	}
}

impl Drop for KeyRatchet {
	fn drop(&mut self) {
		for key in self.keys.values_mut() {
			key.zeroize();
		}
	}
}

fn derive(secret: &[u8], label: &[u8], generation: u32) -> Option<Zeroizing<Vec<u8>>> {
	let length = if label == b"key" { KEY_BYTES } else { SECRET_BYTES };
	let mut mac = Hmac::init(secret, &MessageDigest::sha256()).ok()?;
	mac.update(&(length as u16).to_be_bytes()).ok()?;
	mac.update(&[(LABEL_PREFIX.len() + label.len()) as u8]).ok()?;
	mac.update(LABEL_PREFIX).ok()?;
	mac.update(label).ok()?;
	mac.update(&[GENERATION_CONTEXT_BYTES]).ok()?;
	mac.update(&generation.to_be_bytes()).ok()?;
	mac.update(&[FIRST_BLOCK_COUNTER]).ok()?;
	let out = Zeroizing::new(mac.finalize().ok()?);
	(out.len() == SECRET_BYTES).then_some(out)
}
