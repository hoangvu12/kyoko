mod annex_b;
#[cfg(windows)]
mod byte_stream;
#[cfg(windows)]
mod camera;
mod capture;
mod convert;
mod decode;
mod encode;
mod error;
mod file;
#[cfg(windows)]
mod media_foundation;
#[cfg(windows)]
mod media_foundation_encoder;
#[cfg(windows)]
mod mf;
#[cfg(windows)]
mod mf_send;
mod preview;
mod scale;
#[cfg(windows)]
mod screen;
#[cfg(windows)]
mod source_reader;

use std::time::Duration;

pub use capture::{
	CameraInfo, ScreenKind, ScreenTarget, TestPattern, VideoSource, cameras, open_camera, open_screen,
	screen_thumbnail, screens,
};
pub use convert::{Picture, fit};
pub use decode::H264Decoder;
pub use encode::{EncodedFrame, EncoderConfig, H264Encoder, VideoCodecs, available};
pub use error::VideoError;
pub use file::{ByteSource, FileReader, Packet, SoundTrack, Tracks, VideoTrack};
pub use preview::CameraPreview;
pub use scale::scale_nv12;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoQuality {
	pub width: u32,
	pub height: u32,
	pub fps: u32,
	pub bitrate: u32,
}

impl VideoQuality {
	pub const PRESETS: [Self; 3] = [
		Self {
			width: 1280,
			height: 720,
			fps: 30,
			bitrate: 2_500_000,
		},
		Self {
			width: 854,
			height: 480,
			fps: 30,
			bitrate: 1_200_000,
		},
		Self {
			width: 640,
			height: 360,
			fps: 30,
			bitrate: 800_000,
		},
	];

	pub fn preset(index: u8) -> Self {
		Self::PRESETS
			.get(usize::from(index))
			.copied()
			.unwrap_or(Self::PRESETS[0])
	}

	pub fn layer(&self, quality: u8) -> Self {
		let share = f64::from(quality.clamp(1, 100)) / 100.;
		let even = |value: u32| ((f64::from(value) * share).round() as u32).max(2) & !1;
		let bitrate_share = share * share;
		Self {
			width: even(self.width),
			height: even(self.height),
			fps: self.fps,
			bitrate: (f64::from(self.bitrate) * bitrate_share).round() as u32,
		}
	}
}

impl Default for VideoQuality {
	fn default() -> Self {
		Self::PRESETS[0]
	}
}
