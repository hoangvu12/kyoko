use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use voice::{SoundTarget, open_loopback};

const TONE_HZ: f32 = 440.0;
const TONE_LEVEL: f32 = 0.05;
const PLAY_FOR: Duration = Duration::from_secs(2);

fn main() {
	let device = cpal::default_host()
		.default_output_device()
		.expect("an output device");
	let config = device.default_output_config().expect("an output config").config();
	let rate = config.sample_rate as f32;
	let channels = usize::from(config.channels);
	let mut position = 0u64;
	let stream = device
		.build_output_stream(
			config,
			move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
				for frame in data.chunks_exact_mut(channels) {
					let at = position as f32 / rate;
					let sample = (at * TONE_HZ * std::f32::consts::TAU).sin() * TONE_LEVEL;
					frame.fill(sample);
					position += 1;
				}
			},
			|error| eprintln!("tone stream error: {error}"),
			None,
		)
		.expect("the tone plays");
	let target = SoundTarget::App {
		process: std::process::id(),
	};
	let mut source = open_loopback(target).expect("process loopback opens");
	stream.play().expect("the tone starts");
	let started = Instant::now();
	let mut captured = Vec::new();
	while started.elapsed() < PLAY_FOR {
		source.read(&mut captured).expect("process loopback reads");
	}
	drop(stream);
	let energy: f32 = captured.iter().map(|sample| sample * sample).sum();
	let level = 10.0 * (energy / captured.len().max(1) as f32 + 1e-12).log10();
	let expected = 20.0 * (TONE_LEVEL / std::f32::consts::SQRT_2).log10();
	println!(
		"captured {} stereo frames of this process's own sound at {level:.1} dBFS (tone played at {expected:.1} dBFS)",
		captured.len() / 2
	);
}
