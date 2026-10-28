use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::clip::frames_to_duration;

#[derive(Clone)]
pub struct Feed {
	inner: Arc<Mutex<Inner>>,
}

struct Inner {
	rate: u32,
	queue: VecDeque<(f32, f32)>,
	base: Duration,
	consumed: u64,
	phase: f64,
	playing: bool,
	gain: f32,
	ended: bool,
}

impl Feed {
	pub fn new(rate: u32) -> Self {
		Self {
			inner: Arc::new(Mutex::new(Inner {
				rate: rate.max(1),
				queue: VecDeque::new(),
				base: Duration::ZERO,
				consumed: 0,
				phase: 0.,
				playing: false,
				gain: 1.,
				ended: false,
			})),
		}
	}

	pub fn push(&self, at: Duration, samples: &[f32], channels: u16) {
		let channels = usize::from(channels.max(1));
		let mut inner = self.inner.lock();
		if inner.queue.is_empty() && inner.consumed == 0 {
			inner.base = at;
		}
		inner.ended = false;
		inner
			.queue
			.extend(samples.chunks_exact(channels).map(|frame| match frame {
				[mono] => (*mono, *mono),
				[left, right] => (*left, *right),
				[left, right, rest @ ..] => {
					let extra = rest.iter().sum::<f32>() / rest.len() as f32;
					(((left + extra) * 0.5), ((right + extra) * 0.5))
				}
				[] => (0., 0.),
			}));
	}

	pub fn restart(&self, at: Duration) {
		let mut inner = self.inner.lock();
		inner.queue.clear();
		inner.base = at;
		inner.consumed = 0;
		inner.phase = 0.;
		inner.ended = false;
	}

	pub fn clock(&self) -> Duration {
		let inner = self.inner.lock();
		inner.base + frames_to_duration(inner.consumed as f64 + inner.phase, inner.rate)
	}

	pub fn buffered(&self) -> Duration {
		let inner = self.inner.lock();
		frames_to_duration(inner.queue.len() as f64, inner.rate)
	}

	pub fn is_empty(&self) -> bool {
		self.inner.lock().queue.is_empty()
	}

	pub fn set_playing(&self, playing: bool) {
		self.inner.lock().playing = playing;
	}

	pub fn set_gain(&self, gain: f32) {
		self.inner.lock().gain = if gain.is_finite() { gain.clamp(0., 1.) } else { 0. };
	}

	pub fn finish(&self) {
		self.inner.lock().ended = true;
	}

	pub fn ended(&self) -> bool {
		let inner = self.inner.lock();
		inner.ended && inner.queue.is_empty()
	}

	pub fn mix(&self, out: &mut [f32], channels: u16, rate: u32) {
		let Some(mut inner) = self.inner.try_lock() else {
			return;
		};
		if !inner.playing || rate == 0 {
			return;
		}
		let step = f64::from(inner.rate) / f64::from(rate);
		let gain = inner.gain;
		let channels = usize::from(channels.max(1));
		for frame in out.chunks_exact_mut(channels) {
			let Some(&(left, right)) = inner.queue.front() else {
				return;
			};
			let (next_left, next_right) = inner.queue.get(1).copied().unwrap_or((left, right));
			let along = inner.phase as f32;
			let left = (left + (next_left - left) * along) * gain;
			let right = (right + (next_right - right) * along) * gain;
			match frame {
				[mono] => *mono += (left + right) * 0.5,
				[first, second, ..] => {
					*first += left;
					*second += right;
				}
				[] => {}
			}
			inner.phase += step;
			while inner.phase >= 1. {
				inner.phase -= 1.;
				if inner.queue.pop_front().is_none() {
					inner.phase = 0.;
					return;
				}
				inner.consumed += 1;
			}
		}
	}
}
