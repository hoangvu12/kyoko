use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
	pub width: u32,
	pub height: u32,
	pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
	pub image: DecodedImage,
	pub delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
	pub frames: Vec<Frame>,
	pub loops: Option<u32>,
}

type ReplayFn = dyn Fn(&mut dyn FnMut(DecodedImage) -> bool) + Send + Sync;

#[derive(Clone)]
pub struct Replay(Arc<ReplayFn>);

impl Replay {
	pub fn new(replay: impl Fn(&mut dyn FnMut(DecodedImage) -> bool) + Send + Sync + 'static) -> Self {
		Self(Arc::new(replay))
	}

	pub fn run(&self, sink: &mut dyn FnMut(DecodedImage) -> bool) {
		(self.0)(sink);
	}

	pub fn map(self, change: impl Fn(DecodedImage) -> DecodedImage + Send + Sync + 'static) -> Self {
		Self::new(move |sink| self.run(&mut |image| sink(change(image))))
	}
}

impl fmt::Debug for Replay {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str("Replay")
	}
}

impl PartialEq for Replay {
	fn eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}
}

impl Eq for Replay {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Streamed {
	pub first: DecodedImage,
	pub delays_ms: Vec<u32>,
	pub loops: Option<u32>,
	pub source_bytes: usize,
	pub replay: Replay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
	Still(DecodedImage),
	Animated(Animation),
	Streamed(Streamed),
}

impl Decoded {
	pub fn dimensions(&self) -> (u32, u32) {
		match self {
			Self::Still(image) => (image.width, image.height),
			Self::Animated(animation) => {
				let first = &animation.frames[0].image;
				(first.width, first.height)
			}
			Self::Streamed(streamed) => (streamed.first.width, streamed.first.height),
		}
	}

	pub fn byte_len(&self) -> usize {
		match self {
			Self::Still(image) => image.rgba.len(),
			Self::Animated(animation) => animation.frames.iter().map(|frame| frame.image.rgba.len()).sum(),
			Self::Streamed(streamed) => streamed.first.rgba.len() + streamed.source_bytes,
		}
	}
}
