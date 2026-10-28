#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
	Opus,
}

pub trait FrameTransform: Send {
	fn encrypt(&mut self, codec: Codec, frame: &[u8]) -> Option<Vec<u8>>;
	fn decrypt(&mut self, codec: Codec, ssrc: u32, payload: &[u8]) -> Option<Vec<u8>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Passthrough;

impl FrameTransform for Passthrough {
	fn encrypt(&mut self, _codec: Codec, frame: &[u8]) -> Option<Vec<u8>> {
		Some(frame.to_vec())
	}

	fn decrypt(&mut self, _codec: Codec, _ssrc: u32, payload: &[u8]) -> Option<Vec<u8>> {
		Some(payload.to_vec())
	}
}
