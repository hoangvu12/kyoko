use std::f32::consts::PI;

use realfft::RealFftPlanner;
use realfft::num_complex::Complex32;

const SPEECH_WAV: &[u8] = include_bytes!("../fixtures/speech.wav");
const FIXTURE_RATE: usize = 16_000;
const UPSAMPLE: usize = crate::SAMPLE_RATE / FIXTURE_RATE;
const SINC_HALF_WIDTH: usize = 16;
const SPEECH_PEAK: f32 = 0.5;
const PINK_POLES: [f32; 3] = [0.997, 0.985, 0.95];
const HUM_HZ: f32 = 120.0;
const HUM_HARMONICS: usize = 4;
const HUM_SHARE: f32 = 0.3;
const SILENCE_FLOOR: f32 = 1e-12;

pub fn speech() -> Vec<f32> {
	let pcm = wav_pcm(SPEECH_WAV);
	let mut speech = upsample(&pcm);
	let peak = speech.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
	if peak > 0.0 {
		for sample in &mut speech {
			*sample *= SPEECH_PEAK / peak;
		}
	}
	speech
}

fn wav_pcm(bytes: &[u8]) -> Vec<f32> {
	let mut at = 12;
	while at + 8 <= bytes.len() {
		let id = &bytes[at..at + 4];
		let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]) as usize;
		let body = at + 8;
		if id == b"data" {
			let end = (body + size).min(bytes.len());
			return bytes[body..end]
				.as_chunks::<2>()
				.0
				.iter()
				.map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
				.collect();
		}
		at = body + size + size % 2;
	}
	Vec::new()
}

fn upsample(input: &[f32]) -> Vec<f32> {
	let length = input.len() * UPSAMPLE;
	let taps = SINC_HALF_WIDTH as isize;
	(0..length)
		.map(|index| {
			let position = index as f32 / UPSAMPLE as f32;
			let centre = position.floor() as isize;
			(centre - taps + 1..=centre + taps)
				.filter(|tap| *tap >= 0 && (*tap as usize) < input.len())
				.map(|tap| {
					let distance = position - tap as f32;
					let window = 0.5 + 0.5 * (PI * distance / SINC_HALF_WIDTH as f32).cos();
					input[tap as usize] * sinc(distance) * window
				})
				.sum()
		})
		.collect()
}

fn sinc(x: f32) -> f32 {
	if x.abs() < 1e-6 {
		1.0
	} else {
		(PI * x).sin() / (PI * x)
	}
}

pub fn noise(length: usize, seed: u64) -> Vec<f32> {
	let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
	let mut poles = [0.0f32; 3];
	(0..length)
		.map(|index| {
			state ^= state << 13;
			state ^= state >> 7;
			state ^= state << 17;
			let white = (state >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0;
			for (pole, coefficient) in poles.iter_mut().zip(PINK_POLES) {
				*pole = coefficient * *pole + white * (1.0 - coefficient);
			}
			let pink: f32 = poles.iter().sum::<f32>() + white * 0.05;
			let time = index as f32 / crate::SAMPLE_RATE as f32;
			let hum: f32 = (1..=HUM_HARMONICS)
				.map(|harmonic| (2.0 * PI * HUM_HZ * harmonic as f32 * time).sin() / harmonic as f32)
				.sum();
			pink + hum * HUM_SHARE * 0.05
		})
		.collect()
}

pub struct Clip {
	pub clean: Vec<f32>,
	pub noisy: Vec<f32>,
	pub lead_in: usize,
}

impl Clip {
	pub fn new(snr_db: f32, lead_in: usize, seed: u64) -> Self {
		let speech = speech();
		let noise = noise(lead_in + speech.len(), seed);
		let wanted = power(&speech) / 10f32.powf(snr_db / 10.0);
		let scale = (wanted / power(&noise).max(SILENCE_FLOOR)).sqrt();
		let mut clean = vec![0.0; lead_in];
		clean.extend(&speech);
		let noisy = clean
			.iter()
			.zip(&noise)
			.map(|(clean, noise)| clean + noise * scale)
			.collect();
		Self {
			clean,
			noisy,
			lead_in,
		}
	}

	pub fn settled_noise(&self) -> std::ops::Range<usize> {
		self.lead_in / 2..self.lead_in
	}

	pub fn speech(&self) -> std::ops::Range<usize> {
		self.lead_in..self.clean.len()
	}
}

pub fn power(samples: &[f32]) -> f32 {
	samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32
}

pub fn level_db(samples: &[f32]) -> f32 {
	10.0 * (power(samples) + SILENCE_FLOOR).log10()
}

pub fn mix(speech: &[f32], noise: &[f32], snr_db: f32) -> Vec<f32> {
	let wanted = power(speech) / 10f32.powf(snr_db / 10.0);
	let scale = (wanted / power(noise).max(SILENCE_FLOOR)).sqrt();
	speech
		.iter()
		.zip(noise)
		.map(|(speech, noise)| speech + noise * scale)
		.collect()
}

pub fn si_sdr_db(reference: &[f32], estimate: &[f32]) -> f32 {
	let length = reference.len().min(estimate.len());
	let (reference, estimate) = (&reference[..length], &estimate[..length]);
	let dot: f32 = reference.iter().zip(estimate).map(|(a, b)| a * b).sum();
	let energy: f32 = reference.iter().map(|sample| sample * sample).sum();
	let scale = dot / energy.max(SILENCE_FLOOR);
	let target: f32 = energy * scale * scale;
	let error: f32 = reference
		.iter()
		.zip(estimate)
		.map(|(reference, estimate)| {
			let residual = estimate - reference * scale;
			residual * residual
		})
		.sum();
	10.0 * (target.max(SILENCE_FLOOR) / error.max(SILENCE_FLOOR)).log10()
}

pub fn aligned(reference: &[f32], output: &[f32]) -> Vec<f32> {
	let size = (reference.len() + output.len()).next_power_of_two();
	let mut planner = RealFftPlanner::<f32>::new();
	let forward = planner.plan_fft_forward(size);
	let inverse = planner.plan_fft_inverse(size);
	let spectrum = |samples: &[f32]| {
		let mut padded = forward.make_input_vec();
		padded[..samples.len()].copy_from_slice(samples);
		let mut bins = forward.make_output_vec();
		forward.process(&mut padded, &mut bins).map(|()| bins)
	};
	let (Ok(output_bins), Ok(reference_bins)) = (spectrum(output), spectrum(reference)) else {
		return vec![0.0; reference.len()];
	};
	let mut product: Vec<Complex32> = output_bins
		.iter()
		.zip(&reference_bins)
		.map(|(output, reference)| output * reference.conj())
		.collect();
	let mut correlation = inverse.make_output_vec();
	if inverse.process(&mut product, &mut correlation).is_err() {
		return vec![0.0; reference.len()];
	}
	let best = correlation
		.iter()
		.enumerate()
		.max_by(|a, b| a.1.total_cmp(b.1))
		.map_or(0, |(index, _)| index);
	let lag = if best > size / 2 {
		best as isize - size as isize
	} else {
		best as isize
	};
	(0..reference.len() as isize)
		.map(|index| {
			usize::try_from(index + lag)
				.ok()
				.and_then(|at| output.get(at))
				.copied()
				.unwrap_or(0.0)
		})
		.collect()
}

pub fn delay(reference: &[f32], output: &[f32], max_lag: usize) -> usize {
	(0..=max_lag)
		.max_by(|a, b| correlation(reference, output, *a).total_cmp(&correlation(reference, output, *b)))
		.unwrap_or(0)
}

fn correlation(reference: &[f32], output: &[f32], lag: usize) -> f32 {
	output
		.get(lag..)
		.unwrap_or_default()
		.iter()
		.zip(reference)
		.map(|(a, b)| a * b)
		.sum()
}
