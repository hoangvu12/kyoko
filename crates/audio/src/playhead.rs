use std::collections::VecDeque;
use std::f32::consts::PI;

use crate::clip::Clip;

const WINDOW_SECONDS: f64 = 0.03;
const TOLERANCE_SECONDS: f64 = 0.008;
const CORRELATION_STRIDE: usize = 4;
const SEARCH_STRIDE: usize = 2;

type Frame = (f32, f32);

pub struct Playhead {
	position: f64,
	speed: f32,
	stretch: Option<Stretch>,
	resampler: Resampler,
}

impl Playhead {
	pub fn new(position: f64, speed: f32) -> Self {
		Self {
			position,
			speed: usable(speed),
			stretch: None,
			resampler: Resampler::default(),
		}
	}

	pub fn position(&self) -> f64 {
		self.position
	}

	pub fn speed(&self) -> f32 {
		self.speed
	}

	pub fn seek(&mut self, position: f64) {
		self.position = position.max(0.);
		self.stretch = None;
		self.resampler = Resampler::default();
	}

	pub fn set_speed(&mut self, speed: f32) {
		let speed = usable(speed);
		if speed != self.speed {
			self.speed = speed;
			self.seek(self.position);
		}
	}

	pub fn next(&mut self, clip: &Clip, device_rate: u32) -> Option<Frame> {
		let step = f64::from(clip.rate()) / f64::from(device_rate.max(1));
		let Self {
			position,
			speed,
			stretch,
			resampler,
		} = self;
		resampler.next(step, || source(clip, position, *speed, stretch))
	}
}

fn usable(speed: f32) -> f32 {
	if speed.is_finite() && speed > 0. {
		speed.clamp(0.25, 4.)
	} else {
		1.
	}
}

fn source(clip: &Clip, position: &mut f64, speed: f32, stretch: &mut Option<Stretch>) -> Option<Frame> {
	let frames = clip.frames() as f64;
	if *position >= frames {
		return None;
	}
	if speed == 1. {
		let frame = clip.frame(*position as usize);
		*position += 1.;
		return Some(frame);
	}
	let stretch = stretch.get_or_insert_with(|| Stretch::new(clip.rate(), *position));
	let frame = stretch.next(clip, speed)?;
	*position = (*position + f64::from(speed)).min(frames);
	Some(frame)
}

#[derive(Default)]
struct Resampler {
	phase: f64,
	previous: Frame,
	next: Frame,
	primed: bool,
}

impl Resampler {
	fn next(&mut self, step: f64, mut pull: impl FnMut() -> Option<Frame>) -> Option<Frame> {
		if !self.primed {
			self.previous = pull()?;
			self.next = pull().unwrap_or(self.previous);
			self.phase = 0.;
			self.primed = true;
		}
		while self.phase >= 1. {
			self.previous = self.next;
			self.next = pull()?;
			self.phase -= 1.;
		}
		let amount = self.phase as f32;
		self.phase += step;
		Some((
			self.previous.0 + (self.next.0 - self.previous.0) * amount,
			self.previous.1 + (self.next.1 - self.previous.1) * amount,
		))
	}
}

struct Stretch {
	window: usize,
	hop: usize,
	tolerance: usize,
	analysis: f64,
	previous: Option<usize>,
	tail: Vec<Frame>,
	ready: VecDeque<Frame>,
	weights: Vec<f32>,
}

impl Stretch {
	fn new(rate: u32, position: f64) -> Self {
		let hop = ((f64::from(rate) * WINDOW_SECONDS / 2.) as usize).max(16);
		let window = hop * 2;
		let weights = (0..window)
			.map(|index| 0.5 - 0.5 * (2. * PI * index as f32 / window as f32).cos())
			.collect();
		Self {
			window,
			hop,
			tolerance: (f64::from(rate) * TOLERANCE_SECONDS) as usize,
			analysis: position,
			previous: None,
			tail: vec![(0., 0.); hop],
			ready: VecDeque::with_capacity(hop),
			weights,
		}
	}

	fn next(&mut self, clip: &Clip, speed: f32) -> Option<Frame> {
		if self.ready.is_empty() {
			if self.analysis >= clip.frames() as f64 {
				return None;
			}
			self.block(clip, speed);
		}
		self.ready.pop_front()
	}

	fn block(&mut self, clip: &Clip, speed: f32) {
		let nominal = self.analysis as usize;
		let start = match self.previous {
			Some(previous) => self.best_start(clip, previous + self.hop, nominal),
			None => nominal,
		};
		for index in 0..self.window {
			let weight = self.weights[index];
			let (left, right) = clip.frame(start + index);
			let shaped = (left * weight, right * weight);
			if index < self.hop {
				let (tail_left, tail_right) = self.tail[index];
				self.ready
					.push_back((tail_left + shaped.0, tail_right + shaped.1));
			} else {
				self.tail[index - self.hop] = shaped;
			}
		}
		self.previous = Some(start);
		self.analysis += self.hop as f64 * f64::from(speed);
	}

	fn best_start(&self, clip: &Clip, natural: usize, nominal: usize) -> usize {
		let low = nominal.saturating_sub(self.tolerance);
		let high = nominal + self.tolerance;
		let mut best = (f32::MIN, nominal);
		for candidate in (low..=high).step_by(SEARCH_STRIDE) {
			let (mut dot, mut energy) = (0f32, 0f32);
			for index in (0..self.window).step_by(CORRELATION_STRIDE) {
				let wanted = clip.mono(natural + index);
				let offered = clip.mono(candidate + index);
				dot += wanted * offered;
				energy += offered * offered;
			}
			let score = if energy > f32::EPSILON {
				dot / energy.sqrt()
			} else {
				0.
			};
			if score > best.0 {
				best = (score, candidate);
			}
		}
		best.1
	}
}
