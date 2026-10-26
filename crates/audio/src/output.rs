use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use parking_lot::Mutex;

use crate::deck::Deck;
use crate::error::{AudioError, Result};

pub const SILENT_RATE: u32 = 48_000;
const SILENT_CHANNELS: u16 = 2;
const SILENT_TICK: Duration = Duration::from_millis(10);
const THREAD_NAME: &str = "kyoko-audio";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sink {
	Device,
	Silent,
	Manual,
}

#[derive(Clone)]
pub struct Output {
	inner: Arc<OutputInner>,
}

struct OutputInner {
	sink: Sink,
	deck: Deck,
	control: Mutex<Option<Sender<Control>>>,
	failure: Arc<Mutex<Option<AudioError>>>,
}

#[derive(Debug, Clone, Copy)]
enum Control {
	Run,
	Idle,
}

impl Output {
	pub fn new(sink: Sink, deck: Deck) -> Self {
		Self {
			inner: Arc::new(OutputInner {
				sink,
				deck,
				control: Mutex::new(None),
				failure: Arc::new(Mutex::new(None)),
			}),
		}
	}

	pub fn sink(&self) -> Sink {
		self.inner.sink
	}

	pub fn deck(&self) -> &Deck {
		&self.inner.deck
	}

	pub fn run(&self) {
		*self.inner.failure.lock() = None;
		self.send(Control::Run);
	}

	pub fn idle(&self) {
		self.send(Control::Idle);
	}

	pub fn failure(&self) -> Option<AudioError> {
		self.inner.failure.lock().clone()
	}

	pub fn pump(&self, played: Duration) {
		if self.inner.sink != Sink::Manual {
			return;
		}
		let frames = (played.as_secs_f64() * f64::from(SILENT_RATE)).round() as usize;
		let mut scratch = vec![0f32; frames * usize::from(SILENT_CHANNELS)];
		self.inner.deck.render(&mut scratch, SILENT_CHANNELS, SILENT_RATE);
	}

	fn send(&self, control: Control) {
		if self.inner.sink == Sink::Manual {
			return;
		}
		let mut sender = self.inner.control.lock();
		if sender.is_none() {
			*sender = self.spawn();
		}
		if let Some(channel) = sender.as_ref()
			&& channel.send(control).is_err()
		{
			*sender = None;
		}
	}

	fn spawn(&self) -> Option<Sender<Control>> {
		let (sender, receiver) = mpsc::channel();
		let deck = self.inner.deck.clone();
		let failure = self.inner.failure.clone();
		let sink = self.inner.sink;
		let spawned = thread::Builder::new()
			.name(THREAD_NAME.to_owned())
			.spawn(move || match sink {
				Sink::Device => device_worker(&deck, &receiver, &failure),
				Sink::Silent | Sink::Manual => silent_worker(&deck, &receiver),
			});
		match spawned {
			Ok(_) => Some(sender),
			Err(error) => {
				*self.inner.failure.lock() = Some(AudioError::Device(error.to_string()));
				None
			}
		}
	}
}

fn device_worker(deck: &Deck, receiver: &Receiver<Control>, failure: &Mutex<Option<AudioError>>) {
	let mut stream: Option<Stream> = None;
	while let Ok(control) = receiver.recv() {
		match control {
			Control::Run if stream.is_none() => match open(deck) {
				Ok(opened) => stream = Some(opened),
				Err(error) => {
					tracing::warn!(%error, "no audio output");
					*failure.lock() = Some(error);
					deck.pause();
				}
			},
			Control::Run => {}
			Control::Idle => stream = None,
		}
	}
}

fn silent_worker(deck: &Deck, receiver: &Receiver<Control>) {
	let mut running = false;
	let mut since = Instant::now();
	let mut scratch = Vec::new();
	loop {
		let control = if running {
			match receiver.recv_timeout(SILENT_TICK) {
				Ok(control) => Some(control),
				Err(RecvTimeoutError::Timeout) => None,
				Err(RecvTimeoutError::Disconnected) => return,
			}
		} else {
			match receiver.recv() {
				Ok(control) => Some(control),
				Err(_) => return,
			}
		};
		match control {
			Some(Control::Run) if !running => {
				running = true;
				since = Instant::now();
			}
			Some(Control::Idle) => running = false,
			_ => {}
		}
		if running {
			let due = (since.elapsed().as_secs_f64() * f64::from(SILENT_RATE)) as usize;
			if due > 0 {
				since += Duration::from_secs_f64(due as f64 / f64::from(SILENT_RATE));
				scratch.resize(due * usize::from(SILENT_CHANNELS), 0.);
				deck.render(&mut scratch, SILENT_CHANNELS, SILENT_RATE);
			}
		}
	}
}

fn open(deck: &Deck) -> Result<Stream> {
	let device = cpal::default_host()
		.default_output_device()
		.ok_or_else(|| AudioError::Device("no output device".to_owned()))?;
	let supported = device
		.default_output_config()
		.map_err(|error| AudioError::Device(error.to_string()))?;
	let config = supported.config();
	let stream = match supported.sample_format() {
		SampleFormat::F32 => build::<f32>(&device, config, deck),
		SampleFormat::I16 => build::<i16>(&device, config, deck),
		SampleFormat::U16 => build::<u16>(&device, config, deck),
		SampleFormat::I32 => build::<i32>(&device, config, deck),
		other => Err(AudioError::Device(format!("unsupported sample format {other}"))),
	}?;
	stream
		.play()
		.map_err(|error| AudioError::Device(error.to_string()))?;
	Ok(stream)
}

fn build<T>(device: &cpal::Device, config: StreamConfig, deck: &Deck) -> Result<Stream>
where
	T: SizedSample + FromSample<f32>,
{
	let deck = deck.clone();
	let (channels, rate) = (config.channels, config.sample_rate);
	let mut scratch: Vec<f32> = Vec::new();
	device
		.build_output_stream::<T, _, _>(
			config,
			move |data: &mut [T], _| {
				scratch.resize(data.len(), 0.);
				deck.render(&mut scratch, channels, rate);
				for (out, sample) in data.iter_mut().zip(&scratch) {
					*out = T::from_sample(*sample);
				}
			},
			|error| tracing::warn!(%error, "audio output stream failed"),
			None,
		)
		.map_err(|error| AudioError::Device(error.to_string()))
}
