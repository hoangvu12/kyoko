use std::f32::consts::TAU;

const PERIOD_SECONDS: f32 = 2.6;
const NOTE_SECONDS: f32 = 0.38;
const DECAY_PER_SECOND: f32 = 9.;
const ATTACK_SECONDS: f32 = 0.004;
const OVERTONE: f32 = 0.25;
const LEVEL: f32 = 0.18;
const NOTES: [(f32, f32); 4] = [(0.00, 659.25), (0.20, 830.61), (0.60, 659.25), (0.80, 830.61)];

fn note(at: f32, pitch: f32) -> f32 {
	if !(0. ..NOTE_SECONDS).contains(&at) {
		return 0.;
	}
	let attack = (at / ATTACK_SECONDS).min(1.);
	let envelope = attack * (-DECAY_PER_SECOND * at).exp();
	envelope * ((TAU * pitch * at).sin() + OVERTONE * (TAU * 2. * pitch * at).sin())
}

pub fn ringtone(rate: u32) -> Vec<f32> {
	let rate = rate.max(1) as f32;
	let frames = (PERIOD_SECONDS * rate) as usize;
	(0..frames)
		.map(|index| {
			let at = index as f32 / rate;
			let value: f32 = NOTES.iter().map(|(start, pitch)| note(at - start, *pitch)).sum();
			(value * LEVEL).clamp(-1., 1.)
		})
		.collect()
}
