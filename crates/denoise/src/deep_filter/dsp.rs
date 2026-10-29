use std::f64::consts::PI;
use std::sync::Arc;

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

const MEAN_NORM_START: f32 = -60.0;
const MEAN_NORM_END: f32 = -90.0;
const UNIT_NORM_START: f32 = 0.001;
const UNIT_NORM_END: f32 = 0.0001;
const ENERGY_FLOOR: f32 = 1e-10;
const ERB_SCALE: f32 = 40.0;

pub(crate) struct Stft {
	hop: usize,
	forward: Arc<dyn RealToComplex<f32>>,
	inverse: Arc<dyn ComplexToReal<f32>>,
	window: Vec<f32>,
	norm: f32,
	analysis_memory: Vec<f32>,
	synthesis_memory: Vec<f32>,
	time: Vec<f32>,
	forward_scratch: Vec<Complex32>,
	inverse_scratch: Vec<Complex32>,
}

impl Stft {
	pub fn new(fft_size: usize, hop: usize) -> Self {
		let mut planner = RealFftPlanner::<f32>::new();
		let forward = planner.plan_fft_forward(fft_size);
		let inverse = planner.plan_fft_inverse(fft_size);
		let half = fft_size as f64 / 2.0;
		let window = (0..fft_size)
			.map(|index| {
				let sin = (0.5 * PI * (index as f64 + 0.5) / half).sin();
				(0.5 * PI * sin * sin).sin() as f32
			})
			.collect();
		Self {
			hop,
			forward_scratch: forward.make_scratch_vec(),
			inverse_scratch: inverse.make_scratch_vec(),
			time: forward.make_input_vec(),
			forward,
			inverse,
			window,
			norm: (2 * hop) as f32 / (fft_size * fft_size) as f32,
			analysis_memory: vec![0.0; fft_size - hop],
			synthesis_memory: vec![0.0; fft_size - hop],
		}
	}

	pub fn bins(&self) -> usize {
		self.window.len() / 2 + 1
	}

	pub fn analysis(&mut self, input: &[f32], spectrum: &mut [Complex32]) {
		let overlap = self.analysis_memory.len();
		let (past, present) = self.time.split_at_mut(overlap);
		let (past_window, present_window) = self.window.split_at(overlap);
		for ((slot, sample), weight) in past.iter_mut().zip(&self.analysis_memory).zip(past_window) {
			*slot = sample * weight;
		}
		for ((slot, sample), weight) in present.iter_mut().zip(input).zip(present_window) {
			*slot = sample * weight;
		}
		let kept = overlap - self.hop;
		if kept > 0 {
			self.analysis_memory.rotate_left(self.hop);
		}
		self.analysis_memory[kept..].copy_from_slice(input);
		if self
			.forward
			.process_with_scratch(&mut self.time, spectrum, &mut self.forward_scratch)
			.is_err()
		{
			spectrum.fill(Complex32::default());
			return;
		}
		for bin in spectrum.iter_mut() {
			*bin *= self.norm;
		}
	}

	pub fn synthesis(&mut self, spectrum: &mut [Complex32], output: &mut [f32]) {
		if let Some(first) = spectrum.first_mut() {
			first.im = 0.0;
		}
		if let Some(last) = spectrum.last_mut() {
			last.im = 0.0;
		}
		if self
			.inverse
			.process_with_scratch(spectrum, &mut self.time, &mut self.inverse_scratch)
			.is_err()
		{
			self.time.fill(0.0);
		}
		for (sample, weight) in self.time.iter_mut().zip(&self.window) {
			*sample *= weight;
		}
		let (head, tail) = self.time.split_at(self.hop);
		for ((out, sample), memory) in output.iter_mut().zip(head).zip(&self.synthesis_memory) {
			*out = sample + memory;
		}
		let kept = self.synthesis_memory.len() - self.hop;
		if kept > 0 {
			self.synthesis_memory.rotate_left(self.hop);
		}
		let (added, replaced) = self.synthesis_memory.split_at_mut(kept);
		let (tail_added, tail_replaced) = tail.split_at(kept);
		for (memory, sample) in added.iter_mut().zip(tail_added) {
			*memory += sample;
		}
		replaced.copy_from_slice(tail_replaced);
	}
}

pub(crate) struct Features {
	bands: Vec<usize>,
	alpha: f32,
	mean_norm: Vec<f32>,
	unit_norm: Vec<f32>,
}

impl Features {
	pub fn new(bands: Vec<usize>, deep_bins: usize, alpha: f32) -> Self {
		Self {
			mean_norm: spaced(MEAN_NORM_START, MEAN_NORM_END, bands.len()),
			unit_norm: spaced(UNIT_NORM_START, UNIT_NORM_END, deep_bins),
			bands,
			alpha,
		}
	}

	pub fn erb(&mut self, spectrum: &[Complex32], output: &mut [f32]) {
		let mut start = 0;
		for ((&size, out), state) in self.bands.iter().zip(output.iter_mut()).zip(&mut self.mean_norm) {
			let energy = spectrum[start..start + size]
				.iter()
				.map(Complex32::norm_sqr)
				.sum::<f32>()
				/ size as f32;
			start += size;
			let db = (energy + ENERGY_FLOOR).log10() * 10.0;
			*state = db * (1.0 - self.alpha) + *state * self.alpha;
			*out = (db - *state) / ERB_SCALE;
		}
	}

	pub fn complex(&mut self, spectrum: &[Complex32], output: &mut [Complex32]) {
		for ((bin, out), state) in spectrum.iter().zip(output.iter_mut()).zip(&mut self.unit_norm) {
			*state = bin.norm() * (1.0 - self.alpha) + *state * self.alpha;
			*out = bin / state.sqrt();
		}
	}

	pub fn apply_gains(&self, spectrum: &mut [Complex32], gains: &[f32]) {
		let mut start = 0;
		for (&size, &gain) in self.bands.iter().zip(gains) {
			for bin in &mut spectrum[start..start + size] {
				*bin *= gain;
			}
			start += size;
		}
	}
}

pub(crate) fn erb_bands(sample_rate: usize, fft_size: usize, count: usize, min_width: usize) -> Vec<usize> {
	let bin_width = sample_rate as f32 / fft_size as f32;
	let low = freq_to_erb(0.0);
	let high = freq_to_erb((sample_rate / 2) as f32);
	let step = (high - low) / count as f32;
	let min_width = min_width as i64;
	let mut bands = vec![0usize; count];
	let mut previous = 0i64;
	let mut borrowed = 0i64;
	for (index, band) in bands.iter_mut().enumerate() {
		let edge = (erb_to_freq(low + (index + 1) as f32 * step) / bin_width).round() as i64;
		let mut width = edge - previous - borrowed;
		if width < min_width {
			borrowed = min_width - width;
			width = min_width;
		} else {
			borrowed = 0;
		}
		*band = width.max(0) as usize;
		previous = edge;
	}
	if let Some(last) = bands.last_mut() {
		*last += 1;
	}
	let bins = fft_size / 2 + 1;
	let total: usize = bands.iter().sum();
	if total > bins
		&& let Some(last) = bands.last_mut()
	{
		*last -= total - bins;
	}
	bands
}

pub(crate) fn norm_alpha(sample_rate: usize, hop: usize, tau: f32) -> f32 {
	let alpha = (-(hop as f32 / sample_rate as f32) / tau).exp();
	let mut precision = 3;
	loop {
		let scale = 10f32.powi(precision);
		let rounded = (alpha * scale).round() / scale;
		if rounded < 1.0 {
			return rounded;
		}
		precision += 1;
	}
}

fn freq_to_erb(freq: f32) -> f32 {
	9.265 * (freq / (24.7 * 9.265)).ln_1p()
}

fn erb_to_freq(erb: f32) -> f32 {
	24.7 * 9.265 * ((erb / 9.265).exp() - 1.0)
}

fn spaced(start: f32, end: f32, count: usize) -> Vec<f32> {
	let step = (end - start) / (count.max(2) - 1) as f32;
	(0..count).map(|index| start + index as f32 * step).collect()
}
