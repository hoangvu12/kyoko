use std::sync::Arc;

use dave::DaveCrypto;

use crate::shared::Shared;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
	Opus,
	H264,
}

impl From<Codec> for dave::Codec {
	fn from(codec: Codec) -> Self {
		match codec {
			Codec::Opus => dave::Codec::Opus,
			Codec::H264 => dave::Codec::H264,
		}
	}
}

pub trait FrameTransform: Send {
	fn encrypt(&mut self, codec: Codec, frame: &[u8]) -> Option<Vec<u8>>;
	fn decrypt(&mut self, codec: Codec, ssrc: u32, payload: &[u8]) -> Option<Vec<u8>>;
}

pub(crate) struct DaveTransform {
	crypto: DaveCrypto,
	shared: Arc<Shared>,
}

impl DaveTransform {
	pub fn new(crypto: DaveCrypto, shared: Arc<Shared>) -> Self {
		Self { crypto, shared }
	}
}

impl FrameTransform for DaveTransform {
	fn encrypt(&mut self, codec: Codec, frame: &[u8]) -> Option<Vec<u8>> {
		self.crypto.encrypt(codec.into(), frame)
	}

	fn decrypt(&mut self, codec: Codec, ssrc: u32, payload: &[u8]) -> Option<Vec<u8>> {
		let sender = self.shared.ssrc_users.lock().get(&ssrc).map(|user| user.get());
		self.crypto.decrypt(codec.into(), sender, payload)
	}
}
