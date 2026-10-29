use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use parking_lot::Mutex;

use crate::error::{AudioError, Result};

const THREAD_NAME: &str = "kyoko-cue";
const SLACK: Duration = Duration::from_millis(120);
const STOP_POLL: Duration = Duration::from_millis(20);

pub type Render = fn(u32) -> Vec<f32>;

#[derive(Clone)]
pub struct Cue {
	render: Render,
	looping: bool,
	turn: Arc<Mutex<Turn>>,
}

#[derive(Default)]
struct Turn {
	playing: Option<u64>,
	last: u64,
}

impl Cue {
	pub fn once(render: Render) -> Self {
		Self::new(render, false)
	}

	pub fn looping(render: Render) -> Self {
		Self::new(render, true)
	}

	fn new(render: Render, looping: bool) -> Self {
		Self {
			render,
			looping,
			turn: Arc::default(),
		}
	}

	pub fn is_playing(&self) -> bool {
		self.turn.lock().playing.is_some()
	}

	pub fn play(&self) {
		let id = {
			let mut turn = self.turn.lock();
			if turn.playing.is_some() {
				return;
			}
			turn.last += 1;
			turn.playing = Some(turn.last);
			turn.last
		};
		let cue = self.clone();
		let spawned = thread::Builder::new()
			.name(THREAD_NAME.to_owned())
			.spawn(move || {
				match open(cue.render, cue.looping) {
					Ok((_stream, length)) => cue.wait(id, length),
					Err(error) => tracing::warn!(%error, "the sound could not be played"),
				}
				cue.finish(id);
			});
		if let Err(error) = spawned {
			tracing::warn!(%error, "the sound thread did not start");
			self.finish(id);
		}
	}

	pub fn stop(&self) {
		self.turn.lock().playing = None;
	}

	fn wait(&self, id: u64, length: Duration) {
		let until = (!self.looping).then(|| Instant::now() + length + SLACK);
		while self.turn.lock().playing == Some(id) {
			let left = until.map_or(STOP_POLL, |until| until.saturating_duration_since(Instant::now()));
			if left.is_zero() {
				return;
			}
			thread::sleep(left.min(STOP_POLL));
		}
	}

	fn finish(&self, id: u64) {
		let mut turn = self.turn.lock();
		if turn.playing == Some(id) {
			turn.playing = None;
		}
	}
}

fn open(render: Render, looping: bool) -> Result<(Stream, Duration)> {
	let device = cpal::default_host()
		.default_output_device()
		.ok_or_else(|| AudioError::Device("no output device".to_owned()))?;
	let supported = device
		.default_output_config()
		.map_err(|error| AudioError::Device(error.to_string()))?;
	let config = supported.config();
	let samples: Arc<[f32]> = render(config.sample_rate).into();
	let length = Duration::from_secs_f64(samples.len() as f64 / f64::from(config.sample_rate.max(1)));
	let stream = match supported.sample_format() {
		SampleFormat::F32 => build::<f32>(&device, config, samples, looping),
		SampleFormat::I16 => build::<i16>(&device, config, samples, looping),
		SampleFormat::U16 => build::<u16>(&device, config, samples, looping),
		SampleFormat::I32 => build::<i32>(&device, config, samples, looping),
		other => Err(AudioError::Device(format!("unsupported sample format {other}"))),
	}?;
	stream
		.play()
		.map_err(|error| AudioError::Device(error.to_string()))?;
	Ok((stream, length))
}

fn build<T>(device: &cpal::Device, config: StreamConfig, samples: Arc<[f32]>, looping: bool) -> Result<Stream>
where
	T: SizedSample + FromSample<f32>,
{
	let channels = usize::from(config.channels.max(1));
	let mut position = 0;
	device
		.build_output_stream::<T, _, _>(
			config,
			move |data: &mut [T], _| {
				for frame in data.chunks_exact_mut(channels) {
					if looping && position == samples.len() {
						position = 0;
					}
					let sample = samples.get(position).copied().unwrap_or(0.);
					position = (position + 1).min(samples.len());
					frame.fill(T::from_sample(sample));
				}
			},
			|error| tracing::warn!(%error, "the sound stream failed"),
			None,
		)
		.map_err(|error| AudioError::Device(error.to_string()))
}
