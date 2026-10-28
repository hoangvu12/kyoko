#[cfg(windows)]
mod byte_stream;
mod convert;
mod decode;
mod error;
mod file;
#[cfg(windows)]
mod media_foundation;
#[cfg(windows)]
mod mf;
#[cfg(windows)]
mod source_reader;

use std::time::Duration;

pub use convert::{Picture, fit};
pub use decode::{H264Decoder, available};
pub use error::VideoError;
pub use file::{ByteSource, FileReader, Packet, SoundTrack, Tracks, VideoTrack};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
	pub width: u32,
	pub height: u32,
	pub nv12: Vec<u8>,
	pub timestamp: Duration,
}

impl VideoFrame {
	pub fn expected_len(&self) -> usize {
		let pixels = self.width as usize * self.height as usize;
		pixels + pixels / 2
	}
}
