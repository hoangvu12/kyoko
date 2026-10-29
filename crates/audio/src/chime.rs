use std::f32::consts::{PI, TAU};

const LENGTH_SECONDS: f32 = 0.38;
const FADE_SECONDS: f32 = 0.02;
const ATTACK_SECONDS: f32 = 0.006;
const GLIDE_SECONDS: f32 = 0.025;
const GLIDE_SHARPNESS: f32 = 0.03;
const HARMONIC_GAIN: f32 = 0.1;
const HARMONIC_DECAY_RATIO: f32 = 3.;
const LOW_PASS_HERTZ: f32 = 1800.;
const PEAK_LEVEL: f32 = 0.2;
const NOTES: [Note; 2] = [
	Note {
		hertz: 392.,
		start: 0.,
		decay_seconds: 0.07,
		gain: 1.,
	},
	Note {
		hertz: 523.25,
		start: 0.065,
		decay_seconds: 0.11,
		gain: 0.794,
	},
];

struct Note {
	hertz: f32,
	start: f32,
	decay_seconds: f32,
	gain: f32,
}

impl Note {
	fn sample(&self, at: f32) -> f32 {
		let local = at - self.start;
		if local < 0. {
			return 0.;
		}
		let attack = if local < ATTACK_SECONDS {
			0.5 - 0.5 * (PI * local / ATTACK_SECONDS).cos()
		} else {
			1.
		};
		let remaining = (1. - local / GLIDE_SECONDS).max(0.);
		let glide = GLIDE_SECONDS / 3. * (1. - remaining.powi(3));
		let phase = TAU * self.hertz * (local + GLIDE_SHARPNESS * glide);
		let fundamental = (-local / self.decay_seconds).exp() * phase.sin();
		let harmonic =
			HARMONIC_GAIN * (-local * HARMONIC_DECAY_RATIO / self.decay_seconds).exp() * (2. * phase).sin();
		self.gain * attack * (fundamental + harmonic)
	}
}

pub fn message_chime(rate: u32) -> Vec<f32> {
	let rate = rate.max(1) as f32;
	let frames = (LENGTH_SECONDS * rate) as usize;
	let fade_frames = FADE_SECONDS * rate;
	let smoothing = 1. - (-TAU * LOW_PASS_HERTZ / rate).exp();
	let mut filtered = 0.;
	let mut samples: Vec<f32> = (0..frames)
		.map(|index| {
			let at = index as f32 / rate;
			filtered += smoothing * (NOTES.iter().map(|note| note.sample(at)).sum::<f32>() - filtered);
			let tail = ((frames - 1 - index) as f32 / fade_frames).min(1.);
			filtered * (0.5 - 0.5 * (PI * tail).cos())
		})
		.collect();
	let peak = samples.iter().fold(0f32, |peak, sample| peak.max(sample.abs()));
	if peak > 0. {
		let scale = PEAK_LEVEL / peak;
		samples.iter_mut().for_each(|sample| *sample *= scale);
	}
	samples
}
