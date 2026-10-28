use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle, sleep};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::atomic::AtomicF32;
use crate::audio::{AudioEngine, Rings};
use crate::chain::{CaptureChain, PlaybackChain};
use crate::codec::{FRAME_SAMPLES, STEREO_FRAME_SAMPLES, VoiceDecoder, VoiceEncoder};
use crate::connection::AudioIo;
use crate::denoise::Denoiser;
use crate::error::{Result, VoiceError};
use crate::gate::{LEVEL_FLOOR_DB, decayed_db, rms, to_db};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const PLAYBACK_PRIME_FRAMES: usize = 3;
const DRAIN_TIMEOUT: Duration = Duration::from_millis(500);
const LEVEL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_MANUAL_OUTPUT_FRAMES: usize = 500;
const MAX_VOLUME: f32 = 2.0;

pub struct MicCheckOptions {
	pub audio: AudioIo,
	pub input_volume: f32,
	pub noise_suppression: bool,
}

struct Checked {
	stop: AtomicBool,
	input_volume: AtomicF32,
	noise_suppression: AtomicBool,
	level_db: AtomicF32,
	manual_input: Mutex<VecDeque<Vec<f32>>>,
	manual_output: Mutex<VecDeque<Vec<i16>>>,
}

pub struct MicCheck {
	checked: Arc<Checked>,
	thread: Mutex<Option<JoinHandle<()>>>,
}

struct Devices {
	rings: Arc<Rings>,
	playback: PlaybackChain,
	_engine: AudioEngine,
}

enum Sink {
	Devices(Box<Devices>),
	Manual,
}

struct Loop {
	checked: Arc<Checked>,
	sink: Sink,
	capture: CaptureChain,
	encoder: VoiceEncoder,
	decoder: VoiceDecoder,
	denoiser: Denoiser,
	microphone: Vec<f32>,
	speaker: Vec<f32>,
}

impl MicCheck {
	pub fn start(options: MicCheckOptions, on_level: impl Fn(f32) + Send + 'static) -> Result<Self> {
		let checked = Arc::new(Checked {
			stop: AtomicBool::new(false),
			input_volume: AtomicF32::new(options.input_volume.clamp(0.0, MAX_VOLUME)),
			noise_suppression: AtomicBool::new(options.noise_suppression),
			level_db: AtomicF32::new(LEVEL_FLOOR_DB),
			manual_input: Mutex::new(VecDeque::new()),
			manual_output: Mutex::new(VecDeque::new()),
		});
		let sink = match options.audio {
			AudioIo::Devices { input, output } => open_devices(input, output)?,
			AudioIo::Playback { .. } => return Err(VoiceError::Audio("no input device".to_owned())),
			AudioIo::Manual => Sink::Manual,
		};
		let mut running = Loop {
			checked: checked.clone(),
			sink,
			capture: CaptureChain::default(),
			encoder: VoiceEncoder::new()?,
			decoder: VoiceDecoder::new()?,
			denoiser: Denoiser::new(),
			microphone: vec![0.0; FRAME_SAMPLES],
			speaker: vec![0.0; STEREO_FRAME_SAMPLES],
		};
		let thread = thread::Builder::new()
			.name("voice-mic-check".to_owned())
			.spawn(move || running.run(&on_level))
			.map_err(|error| VoiceError::Audio(error.to_string()))?;
		Ok(Self {
			checked,
			thread: Mutex::new(Some(thread)),
		})
	}

	pub fn set_input_volume(&self, volume: f32) {
		self.checked.input_volume.store(volume.clamp(0.0, MAX_VOLUME));
	}

	pub fn set_noise_suppression(&self, enabled: bool) {
		self.checked.noise_suppression.store(enabled, Ordering::Relaxed);
	}

	pub fn level_db(&self) -> f32 {
		self.checked.level_db.load()
	}

	pub fn feed_input(&self, pcm: &[i16]) {
		let frame = pcm
			.iter()
			.map(|sample| f32::from(*sample) / f32::from(i16::MAX))
			.collect();
		self.checked.manual_input.lock().push_back(frame);
	}

	pub fn take_output(&self) -> Vec<Vec<i16>> {
		self.checked.manual_output.lock().drain(..).collect()
	}

	pub fn stop(&self) {
		self.checked.stop.store(true, Ordering::Relaxed);
		let thread = self.thread.lock().take();
		if let Some(thread) = thread {
			let _ = thread.join();
		}
	}
}

impl Drop for MicCheck {
	fn drop(&mut self) {
		self.stop();
	}
}

fn open_devices(input: Option<String>, output: Option<String>) -> Result<Sink> {
	let rings = Arc::new(Rings::new());
	let engine = AudioEngine::start(rings.clone(), input, output)?;
	if rings.capture_rate() == 0 {
		return Err(VoiceError::Audio("no input device".to_owned()));
	}
	if rings.playback_rate() == 0 {
		return Err(VoiceError::Audio("no output device".to_owned()));
	}
	let mut playback = PlaybackChain::new(rings.clone());
	let silence = vec![0.0; STEREO_FRAME_SAMPLES];
	for _ in 0..PLAYBACK_PRIME_FRAMES {
		playback.push(&silence)?;
	}
	Ok(Sink::Devices(Box::new(Devices {
		rings,
		playback,
		_engine: engine,
	})))
}

impl Loop {
	fn run(&mut self, on_level: &dyn Fn(f32)) {
		let mut next_level = Instant::now() + LEVEL_INTERVAL;
		let mut peak = LEVEL_FLOOR_DB;
		while !self.checked.stop.load(Ordering::Relaxed) {
			if let Err(error) = self.pull() {
				tracing::warn!(%error, "mic check capture failed");
			}
			while self.capture.take_frame(&mut self.microphone) {
				peak = peak.max(self.process());
			}
			if Instant::now() >= next_level {
				let level = decayed_db(peak, self.checked.level_db.load());
				self.checked.level_db.store(level);
				on_level(level);
				peak = LEVEL_FLOOR_DB;
				next_level += LEVEL_INTERVAL;
			}
			sleep(POLL_INTERVAL);
		}
		self.drain();
	}

	fn pull(&mut self) -> Result<()> {
		match &self.sink {
			Sink::Devices(devices) => self.capture.pull(&devices.rings),
			Sink::Manual => {
				let queued: Vec<Vec<f32>> = self.checked.manual_input.lock().drain(..).collect();
				for frame in queued {
					self.capture.inject(&frame);
				}
				Ok(())
			}
		}
	}

	fn process(&mut self) -> f32 {
		let gain = self.checked.input_volume.load();
		for sample in &mut self.microphone {
			*sample *= gain;
		}
		if self.checked.noise_suppression.load(Ordering::Relaxed) {
			self.denoiser.process(&mut self.microphone);
		}
		let level = to_db(rms(&self.microphone));
		if let Err(error) = self.play_back() {
			tracing::warn!(%error, "mic check playback failed");
		}
		level
	}

	fn play_back(&mut self) -> Result<()> {
		let packet = self.encoder.encode(&self.microphone)?;
		self.decoder.decode(packet, false, &mut self.speaker)?;
		match &mut self.sink {
			Sink::Devices(devices) => devices.playback.push(&self.speaker),
			Sink::Manual => {
				let pcm = self
					.speaker
					.iter()
					.map(|sample| (sample * f32::from(i16::MAX)) as i16)
					.collect();
				let mut output = self.checked.manual_output.lock();
				if output.len() >= MAX_MANUAL_OUTPUT_FRAMES {
					output.pop_front();
				}
				output.push_back(pcm);
				Ok(())
			}
		}
	}

	fn drain(&self) {
		let Sink::Devices(devices) = &self.sink else {
			return;
		};
		let deadline = Instant::now() + DRAIN_TIMEOUT;
		while devices.rings.playback_len() > 0 && Instant::now() < deadline {
			sleep(POLL_INTERVAL);
		}
	}
}
