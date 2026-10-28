use std::io;
use std::sync::Arc;
use std::time::Duration;

use crate::{VideoError, VideoFrame};

pub trait ByteSource: Send + Sync {
	fn length(&self) -> io::Result<Option<u64>>;

	fn read_at(&self, position: u64, buffer: &mut [u8]) -> io::Result<usize>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoTrack {
	pub width: u32,
	pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundTrack {
	pub rate: u32,
	pub channels: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tracks {
	pub video: Option<VideoTrack>,
	pub sound: Option<SoundTrack>,
	pub duration: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Packet {
	Video(VideoFrame),
	Audio { timestamp: Duration, samples: Vec<f32> },
	End,
}

pub struct FileReader {
	#[cfg(windows)]
	inner: crate::source_reader::SourceReader,
}

impl FileReader {
	#[cfg(windows)]
	pub fn open(source: Arc<dyn ByteSource>, name: &str) -> Result<Self, VideoError> {
		Ok(Self {
			inner: crate::source_reader::SourceReader::open(source, name)?,
		})
	}

	#[cfg(not(windows))]
	pub fn open(_source: Arc<dyn ByteSource>, _name: &str) -> Result<Self, VideoError> {
		Err(VideoError::Unsupported)
	}

	#[cfg(windows)]
	pub fn tracks(&self) -> Tracks {
		self.inner.tracks()
	}

	#[cfg(not(windows))]
	pub fn tracks(&self) -> Tracks {
		Tracks::default()
	}

	#[cfg(windows)]
	pub fn packet(&mut self) -> Result<Packet, VideoError> {
		self.inner.packet()
	}

	#[cfg(not(windows))]
	pub fn packet(&mut self) -> Result<Packet, VideoError> {
		Err(VideoError::Unsupported)
	}

	#[cfg(windows)]
	pub fn seek(&mut self, to: Duration) -> Result<(), VideoError> {
		self.inner.seek(to)
	}

	#[cfg(not(windows))]
	pub fn seek(&mut self, _to: Duration) -> Result<(), VideoError> {
		Err(VideoError::Unsupported)
	}
}
