mod convert;
mod decode;
mod error;
pub(crate) mod hub;
#[cfg(windows)]
mod media_foundation;
#[cfg(windows)]
mod mf;
pub(crate) mod receive;
pub(crate) mod rtp;
pub(crate) mod wants;

use std::sync::Arc;
use std::time::Duration;

use protocol::Snowflake;

pub use convert::{Picture, fit};
pub use decode::{H264Decoder, available};
pub use error::VideoError;
pub use hub::VideoStats;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoStream {
	pub user_id: Snowflake,
	pub ssrc: u32,
}

pub trait VideoSink: Send + Sync {
	fn frame(&self, stream: VideoStream, frame: Arc<VideoFrame>);

	fn ended(&self, _user_id: Snowflake) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoWant {
	pub user_id: Snowflake,
	pub quality: u8,
	pub pixels: u64,
}
