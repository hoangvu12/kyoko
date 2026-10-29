use std::time::{Duration, Instant};

use super::{VideoError, VideoFrame, VideoQuality};

const PATTERN_BARS: [(u8, u8, u8); 6] = [
	(180, 128, 128),
	(168, 44, 136),
	(145, 147, 44),
	(106, 85, 212),
	(81, 90, 240),
	(41, 240, 110),
];
const PATTERN_STEP: usize = 4;

pub trait VideoSource: Send {
	fn next_frame(&mut self) -> Result<VideoFrame, VideoError>;

	fn size(&self) -> (u32, u32);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraInfo {
	pub id: String,
	pub name: String,
}

#[cfg(windows)]
pub fn cameras() -> Result<Vec<CameraInfo>, VideoError> {
	super::camera::CameraSource::list()
}

#[cfg(not(windows))]
pub fn cameras() -> Result<Vec<CameraInfo>, VideoError> {
	Err(VideoError::Unsupported)
}

#[cfg(windows)]
pub fn open_camera(id: Option<&str>, quality: VideoQuality) -> Result<Box<dyn VideoSource>, VideoError> {
	let id = match id {
		Some(id) => id.to_owned(),
		None => cameras()?.into_iter().next().ok_or(VideoError::NotFound)?.id,
	};
	let source = super::camera::CameraSource::open(&id, quality.width, quality.height, quality.fps)?;
	Ok(Box::new(source))
}

#[cfg(not(windows))]
pub fn open_camera(_id: Option<&str>, _quality: VideoQuality) -> Result<Box<dyn VideoSource>, VideoError> {
	Err(VideoError::Unsupported)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenKind {
	Screen,
	Window,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenTarget {
	pub kind: ScreenKind,
	pub id: u64,
	pub name: String,
	pub width: u32,
	pub height: u32,
}

#[cfg(windows)]
pub fn screens() -> Result<Vec<ScreenTarget>, VideoError> {
	Ok(super::screen::ScreenSource::list())
}

#[cfg(not(windows))]
pub fn screens() -> Result<Vec<ScreenTarget>, VideoError> {
	Err(VideoError::Unsupported)
}

#[cfg(windows)]
pub fn open_screen(target: &ScreenTarget, quality: VideoQuality) -> Result<Box<dyn VideoSource>, VideoError> {
	Ok(Box::new(super::screen::ScreenSource::open(
		target, quality, true,
	)?))
}

#[cfg(not(windows))]
pub fn open_screen(
	_target: &ScreenTarget,
	_quality: VideoQuality,
) -> Result<Box<dyn VideoSource>, VideoError> {
	Err(VideoError::Unsupported)
}

#[cfg(windows)]
pub fn screen_thumbnail(target: &ScreenTarget, width: u32, height: u32) -> Result<VideoFrame, VideoError> {
	let quality = VideoQuality {
		width,
		height,
		..VideoQuality::default()
	};
	super::screen::ScreenSource::open(target, quality, false)?.next_frame()
}

#[cfg(not(windows))]
pub fn screen_thumbnail(_target: &ScreenTarget, _width: u32, _height: u32) -> Result<VideoFrame, VideoError> {
	Err(VideoError::Unsupported)
}

pub struct TestPattern {
	width: u32,
	height: u32,
	gap: Duration,
	started: Option<Instant>,
	frames: u32,
}

impl TestPattern {
	pub fn new(quality: VideoQuality) -> Self {
		Self {
			width: quality.width.max(2) & !1,
			height: quality.height.max(2) & !1,
			gap: Duration::from_secs(1) / quality.fps.max(1),
			started: None,
			frames: 0,
		}
	}

	fn paint(&self) -> Vec<u8> {
		let width = self.width as usize;
		let height = self.height as usize;
		let shift = self.frames as usize * PATTERN_STEP;
		let bar = |column: usize| {
			PATTERN_BARS[((column + shift) * PATTERN_BARS.len() / width) % PATTERN_BARS.len()]
		};
		let mut nv12 = vec![0u8; width * height * 3 / 2];
		let (luma, chroma) = nv12.split_at_mut(width * height);
		for row in luma.chunks_exact_mut(width) {
			for (column, pixel) in row.iter_mut().enumerate() {
				*pixel = bar(column).0;
			}
		}
		for row in chroma.chunks_exact_mut(width) {
			for (pair, pixel) in row.as_chunks_mut::<2>().0.iter_mut().enumerate() {
				let (_, u, v) = bar(pair * 2);
				*pixel = [u, v];
			}
		}
		nv12
	}
}

impl VideoSource for TestPattern {
	fn next_frame(&mut self) -> Result<VideoFrame, VideoError> {
		let started = *self.started.get_or_insert_with(Instant::now);
		let due = started + self.gap * self.frames;
		let now = Instant::now();
		if due > now {
			std::thread::sleep(due - now);
		}
		let frame = VideoFrame {
			width: self.width,
			height: self.height,
			nv12: self.paint(),
			timestamp: self.gap * self.frames,
		};
		self.frames = self.frames.wrapping_add(1);
		Ok(frame)
	}

	fn size(&self) -> (u32, u32) {
		(self.width, self.height)
	}
}
