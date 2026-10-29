use sonora::config::{
	AdaptiveDigital, EchoCanceller, GainController2, HighPassFilter, MaxProcessingRate, Pipeline,
};
use sonora::{AudioProcessing, Config, StreamConfig};
use tracing::warn;

use crate::codec::SAMPLE_RATE;

const CHUNK: usize = SAMPLE_RATE as usize / 100;
const MONO: StreamConfig = StreamConfig::new(SAMPLE_RATE, 1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Processing {
	pub echo_cancellation: bool,
	pub auto_gain_control: bool,
}

impl Processing {
	pub const OFF: Processing = Processing {
		echo_cancellation: false,
		auto_gain_control: false,
	};

	pub fn code(self) -> u8 {
		u8::from(self.echo_cancellation) | u8::from(self.auto_gain_control) << 1
	}

	pub fn from_code(code: u8) -> Self {
		Self {
			echo_cancellation: code & 1 != 0,
			auto_gain_control: code & 2 != 0,
		}
	}
}

#[derive(Default)]
pub struct Processor {
	echo: Option<AudioProcessing>,
	gain: Option<AudioProcessing>,
	mono: Vec<f32>,
	output: Vec<f32>,
}

impl Processor {
	pub fn render(&mut self, wanted: Processing, stereo: &[f32]) {
		if !wanted.echo_cancellation {
			self.echo = None;
			return;
		}
		let echo = self.echo.get_or_insert_with(echo_canceller);
		self.mono.clear();
		self.mono.extend(
			stereo
				.as_chunks::<2>()
				.0
				.iter()
				.map(|[left, right]| (left + right) * 0.5),
		);
		self.output.resize(CHUNK, 0.0);
		for chunk in self.mono.as_chunks::<CHUNK>().0 {
			if let Err(error) = echo.process_render_f32(&[chunk.as_slice()], &mut [&mut self.output[..]]) {
				warn!(?error, "the echo canceller rejected a playback frame");
			}
		}
	}

	pub fn cancel_echo(&mut self, wanted: Processing, delay_ms: i32, frame: &mut [f32]) -> bool {
		if !wanted.echo_cancellation {
			self.echo = None;
			return false;
		}
		let echo = self.echo.get_or_insert_with(echo_canceller);
		let _ = echo.set_stream_delay_ms(delay_ms);
		capture(echo, &mut self.output, frame);
		true
	}

	pub fn control_gain(&mut self, wanted: Processing, frame: &mut [f32]) -> bool {
		if !wanted.auto_gain_control {
			self.gain = None;
			return false;
		}
		let gain = self.gain.get_or_insert_with(gain_controller);
		capture(gain, &mut self.output, frame);
		true
	}
}

fn capture(processing: &mut AudioProcessing, output: &mut Vec<f32>, frame: &mut [f32]) {
	output.resize(CHUNK, 0.0);
	for chunk in frame.as_chunks_mut::<CHUNK>().0 {
		match processing.process_capture_f32(&[chunk.as_slice()], &mut [&mut output[..]]) {
			Ok(()) => chunk.copy_from_slice(output),
			Err(error) => warn!(?error, "audio processing rejected a microphone frame"),
		}
	}
}

fn pipeline() -> Pipeline {
	Pipeline {
		maximum_internal_processing_rate: MaxProcessingRate::Rate48kHz,
		..Pipeline::default()
	}
}

fn built(config: Config) -> AudioProcessing {
	AudioProcessing::builder()
		.config(config)
		.capture_config(MONO)
		.render_config(MONO)
		.build()
}

fn echo_canceller() -> AudioProcessing {
	built(Config {
		pipeline: pipeline(),
		high_pass_filter: Some(HighPassFilter::default()),
		echo_canceller: Some(EchoCanceller::default()),
		..Config::default()
	})
}

fn gain_controller() -> AudioProcessing {
	built(Config {
		pipeline: pipeline(),
		gain_controller2: Some(GainController2 {
			adaptive_digital: Some(AdaptiveDigital::default()),
			..GainController2::default()
		}),
		..Config::default()
	})
}
