#[cfg(windows)]
mod loopback;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::{debug, warn};

use crate::codec::SAMPLE_RATE;
use crate::error::{Result, VoiceError};

const RING_LIMIT: usize = SAMPLE_RATE as usize * 2;
const FEED_LIMIT_FRAMES: usize = 5;
const TONE_CHUNK: Duration = Duration::from_millis(10);
const TONE_CHUNK_FRAMES: usize = SAMPLE_RATE as usize / 100;
const FAILURE_BACKOFF: Duration = Duration::from_millis(250);
const MAX_CONSECUTIVE_FAILURES: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundTarget {
	Everything { except: u32 },
	App { process: u32 },
}

pub trait SoundSource: Send {
	fn read(&mut self, stereo: &mut Vec<f32>) -> Result<()>;
}

#[cfg(windows)]
pub fn open_loopback(target: SoundTarget) -> Result<Box<dyn SoundSource>> {
	Ok(Box::new(loopback::ProcessLoopback::open(target)?))
}

#[cfg(not(windows))]
pub fn open_loopback(_target: SoundTarget) -> Result<Box<dyn SoundSource>> {
	Err(VoiceError::Audio("sound capture needs Windows".to_owned()))
}

pub struct ToneSource {
	hz: f32,
	amplitude: f32,
	position: u64,
	next: Instant,
}

impl ToneSource {
	pub fn new(hz: f32, amplitude: f32) -> Self {
		Self {
			hz,
			amplitude,
			position: 0,
			next: Instant::now(),
		}
	}
}

impl SoundSource for ToneSource {
	fn read(&mut self, stereo: &mut Vec<f32>) -> Result<()> {
		let now = Instant::now();
		if self.next > now {
			thread::sleep(self.next - now);
		}
		self.next += TONE_CHUNK;
		for _ in 0..TONE_CHUNK_FRAMES {
			let at = self.position as f32 / SAMPLE_RATE as f32;
			let sample = (at * self.hz * std::f32::consts::TAU).sin() * self.amplitude;
			stereo.extend([sample, sample]);
			self.position += 1;
		}
		Ok(())
	}
}

#[derive(Default)]
pub(crate) struct SoundRing {
	samples: Mutex<VecDeque<f32>>,
}

impl SoundRing {
	fn push(&self, stereo: &[f32]) {
		let mut ring = self.samples.lock();
		ring.extend(stereo);
		let excess = ring.len().saturating_sub(RING_LIMIT);
		ring.drain(..excess);
	}

	pub fn drain_into(&self, into: &mut Vec<f32>) {
		into.extend(self.samples.lock().drain(..));
	}

	pub fn clear(&self) {
		self.samples.lock().clear();
	}
}

pub(crate) struct SoundCapture {
	stop: Arc<AtomicBool>,
	thread: Option<JoinHandle<()>>,
}

impl SoundCapture {
	pub fn spawn(mut source: Box<dyn SoundSource>, ring: Arc<SoundRing>) -> Result<Self> {
		let stop = Arc::new(AtomicBool::new(false));
		let stopped = stop.clone();
		let thread = thread::Builder::new()
			.name("stream-sound".to_owned())
			.spawn(move || {
				let mut chunk = Vec::new();
				let mut failures = 0;
				while !stopped.load(Ordering::Acquire) {
					chunk.clear();
					match source.read(&mut chunk) {
						Ok(()) => {
							failures = 0;
							ring.push(&chunk);
						}
						Err(error) => {
							failures += 1;
							warn!(%error, "stream sound was not captured");
							if failures >= MAX_CONSECUTIVE_FAILURES {
								break;
							}
							thread::sleep(FAILURE_BACKOFF);
						}
					}
				}
				debug!("stream sound capture stopped");
			})
			.map_err(|error| VoiceError::Audio(error.to_string()))?;
		Ok(Self {
			stop,
			thread: Some(thread),
		})
	}
}

impl Drop for SoundCapture {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::Release);
		self.thread.take();
	}
}

pub(crate) struct FedFrame {
	pub stereo: Vec<f32>,
	pub sources: Vec<u32>,
}

#[derive(Clone, Default)]
pub struct PlaybackFeed(Arc<Mutex<VecDeque<FedFrame>>>);

impl PlaybackFeed {
	pub(crate) fn push(&self, stereo: &[f32], sources: Vec<u32>) {
		let mut frames = self.0.lock();
		if frames.len() >= FEED_LIMIT_FRAMES {
			frames.pop_front();
		}
		frames.push_back(FedFrame {
			stereo: stereo.to_vec(),
			sources,
		});
	}

	pub(crate) fn pop(&self) -> Option<FedFrame> {
		self.0.lock().pop_front()
	}
}
