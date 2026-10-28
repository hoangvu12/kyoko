use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use parking_lot::Mutex;

use crate::cryptor::{Decryptor, Encryptor, TRANSITION_EXPIRY};
use crate::frame::{self, SILENCE};
use crate::ratchet::KeyRatchet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
	Opus,
	H264,
}

#[derive(Debug, Default)]
pub struct DaveStats {
	encrypted: AtomicU64,
	decrypted: AtomicU64,
	rejected: AtomicU64,
	passed: AtomicU64,
	enabled: AtomicBool,
}

impl DaveStats {
	pub fn encrypted(&self) -> u64 {
		self.encrypted.load(Ordering::Relaxed)
	}

	pub fn decrypted(&self) -> u64 {
		self.decrypted.load(Ordering::Relaxed)
	}

	pub fn rejected(&self) -> u64 {
		self.rejected.load(Ordering::Relaxed)
	}

	pub fn passed(&self) -> u64 {
		self.passed.load(Ordering::Relaxed)
	}

	pub fn enabled(&self) -> bool {
		self.enabled.load(Ordering::Relaxed)
	}
}

enum Sending {
	Plain,
	Blocked,
	Encrypt(Box<Encryptor>),
}

#[derive(Clone, Copy)]
enum Passthrough {
	Always,
	Until(Instant),
	Never,
}

impl Passthrough {
	fn allows(self, now: Instant) -> bool {
		match self {
			Self::Always => true,
			Self::Until(until) => until > now,
			Self::Never => false,
		}
	}

	fn limit(self, until: Instant) -> Self {
		match self {
			Self::Always => Self::Until(until),
			Self::Until(current) => Self::Until(current.min(until)),
			Self::Never => Self::Never,
		}
	}
}

pub(crate) struct FrameCrypto {
	send: Sending,
	receive: Passthrough,
	decryptors: HashMap<u64, Decryptor>,
	stats: Arc<DaveStats>,
}

impl FrameCrypto {
	pub fn new(stats: Arc<DaveStats>) -> Self {
		Self {
			send: Sending::Plain,
			receive: Passthrough::Always,
			decryptors: HashMap::new(),
			stats,
		}
	}

	pub fn reset(&mut self, blocked: bool) {
		self.set_send(if blocked { Sending::Blocked } else { Sending::Plain });
		self.receive = Passthrough::Always;
		self.decryptors.clear();
	}

	pub fn install_senders(&mut self, ratchets: Vec<(u64, KeyRatchet)>) {
		let now = Instant::now();
		self.decryptors
			.retain(|user, _| ratchets.iter().any(|(member, _)| member == user));
		for (user, ratchet) in ratchets {
			self.decryptors.entry(user).or_default().install(ratchet, now);
		}
	}

	pub fn start_sending(&mut self, ratchet: KeyRatchet) {
		let continuing = matches!(&self.send, Sending::Encrypt(encryptor) if encryptor.same_domain(&ratchet));
		if !continuing {
			self.set_send(Sending::Encrypt(Box::new(Encryptor::new(ratchet))));
		}
		self.receive = self.receive.limit(Instant::now() + TRANSITION_EXPIRY);
	}

	pub fn block_sending(&mut self) {
		self.set_send(Sending::Blocked);
	}

	pub fn stop_sending(&mut self) {
		self.set_send(Sending::Plain);
		self.retire();
	}

	pub fn allow_plain_receive(&mut self) {
		self.receive = Passthrough::Always;
	}

	fn retire(&mut self) {
		let now = Instant::now();
		for decryptor in self.decryptors.values_mut() {
			decryptor.retire(now);
		}
	}

	fn seal(&mut self, codec: Codec, frame: &[u8]) -> Option<Vec<u8>> {
		if codec == Codec::Opus && frame == SILENCE {
			self.stats.passed.fetch_add(1, Ordering::Relaxed);
			return Some(frame.to_vec());
		}
		match &mut self.send {
			Sending::Plain => Some(frame.to_vec()),
			Sending::Blocked => None,
			Sending::Encrypt(encryptor) => {
				let sealed = encryptor.seal(codec, frame);
				if sealed.is_some() {
					self.stats.encrypted.fetch_add(1, Ordering::Relaxed);
				}
				sealed
			}
		}
	}

	fn open(&mut self, codec: Codec, sender: Option<u64>, payload: &[u8]) -> Option<Vec<u8>> {
		if codec == Codec::Opus && payload == SILENCE {
			return Some(payload.to_vec());
		}
		let now = Instant::now();
		let opened = match frame::parse(payload) {
			None => self.receive.allows(now).then(|| payload.to_vec()),
			Some(parsed) => {
				let decrypted = sender
					.and_then(|user| self.decryptors.get_mut(&user))
					.and_then(|decryptor| decryptor.open(&parsed, now));
				if decrypted.is_some() {
					self.stats.decrypted.fetch_add(1, Ordering::Relaxed);
				}
				decrypted
			}
		};
		if opened.is_none() {
			self.stats.rejected.fetch_add(1, Ordering::Relaxed);
		}
		opened
	}

	fn set_send(&mut self, send: Sending) {
		self.stats
			.enabled
			.store(matches!(send, Sending::Encrypt(_)), Ordering::Relaxed);
		self.send = send;
	}
}

#[derive(Clone)]
pub struct DaveCrypto(pub(crate) Arc<Mutex<FrameCrypto>>);

impl DaveCrypto {
	pub fn encrypt(&self, codec: Codec, frame: &[u8]) -> Option<Vec<u8>> {
		self.0.lock().seal(codec, frame)
	}

	pub fn decrypt(&self, codec: Codec, sender: Option<u64>, payload: &[u8]) -> Option<Vec<u8>> {
		self.0.lock().open(codec, sender, payload)
	}
}
