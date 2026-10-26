use std::time::Duration;

use crate::error::{AudioError, Result};

pub const MAX_CHANNELS: u16 = 2;

#[derive(Debug)]
pub struct Clip {
	rate: u32,
	channels: u16,
	samples: Box<[i16]>,
}

impl Clip {
	pub fn new(rate: u32, channels: u16, samples: Vec<i16>) -> Result<Self> {
		if rate == 0 || !(1..=MAX_CHANNELS).contains(&channels) {
			return Err(AudioError::Decode(format!(
				"unusable stream: {rate} Hz, {channels} channels"
			)));
		}
		let mut samples = samples;
		samples.truncate(samples.len() - samples.len() % usize::from(channels));
		Ok(Self {
			rate,
			channels,
			samples: samples.into_boxed_slice(),
		})
	}

	pub fn rate(&self) -> u32 {
		self.rate
	}

	pub fn channels(&self) -> u16 {
		self.channels
	}

	pub fn frames(&self) -> usize {
		self.samples.len() / usize::from(self.channels)
	}

	pub fn duration(&self) -> Duration {
		frames_to_duration(self.frames() as f64, self.rate)
	}

	pub fn frame(&self, index: usize) -> (f32, f32) {
		let channels = usize::from(self.channels);
		let start = index * channels;
		match self.samples.get(start..start + channels) {
			Some([mono]) => {
				let value = to_float(*mono);
				(value, value)
			}
			Some([left, right]) => (to_float(*left), to_float(*right)),
			_ => (0., 0.),
		}
	}

	pub fn mono(&self, index: usize) -> f32 {
		let (left, right) = self.frame(index);
		(left + right) * 0.5
	}
}

pub fn frames_to_duration(frames: f64, rate: u32) -> Duration {
	if rate == 0 || !frames.is_finite() || frames <= 0. {
		return Duration::ZERO;
	}
	Duration::from_secs_f64(frames / f64::from(rate))
}

fn to_float(sample: i16) -> f32 {
	f32::from(sample) / 32_768.
}
