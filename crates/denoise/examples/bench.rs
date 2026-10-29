use std::time::{Duration, Instant};

use denoise::testing::{Clip, delay, level_db, mix, noise, si_sdr_db, speech};
use denoise::{NoiseSuppressor, Suppression};

const FRAME: usize = 960;
const PASSES: usize = 5;
const SNR_DB: [f32; 3] = [0.0, 5.0, 10.0];
const MAX_LAG: usize = 4_800;
const LEAD_IN: usize = 72_000;

fn main() {
	for suppression in [Suppression::HighQuality, Suppression::LowCpu] {
		let started = Instant::now();
		if let Err(error) = denoise::load(suppression) {
			eprintln!("{suppression:?} did not load: {error}");
			continue;
		}
		println!(
			"{suppression:?}: loaded in {:.0} ms",
			started.elapsed().as_secs_f64() * 1000.0
		);
		timing(suppression);
		for snr in SNR_DB {
			quality(suppression, snr, false);
		}
		quality(suppression, 0.0, true);
	}
}

fn timing(suppression: Suppression) {
	let clean = speech();
	let noisy = mix(&clean, &noise(clean.len(), 7), 5.0);
	let mut suppressor = NoiseSuppressor::new();
	let mut hops: Vec<Duration> = Vec::new();
	for _ in 0..PASSES {
		for chunk in noisy.as_chunks::<FRAME>().0 {
			let mut frame = chunk.to_vec();
			let started = Instant::now();
			let used = suppressor.process(suppression, &mut frame);
			hops.push(started.elapsed() / 2);
			assert_eq!(used, Some(suppression));
		}
	}
	hops.sort();
	let mean = hops.iter().sum::<Duration>() / hops.len() as u32;
	let at = |share: f64| hops[((hops.len() - 1) as f64 * share) as usize];
	println!(
		"  per 10 ms hop over {} hops: mean {:.3} ms, median {:.3} ms, p99 {:.3} ms, max {:.3} ms",
		hops.len(),
		ms(mean),
		ms(at(0.5)),
		ms(at(0.99)),
		ms(at(1.0))
	);
}

fn quality(suppression: Suppression, snr_db: f32, after_tone: bool) {
	let clip = Clip::new(snr_db, LEAD_IN, 11);
	let mut suppressor = NoiseSuppressor::new();
	if after_tone {
		for index in 0..60 {
			let mut frame: Vec<f32> = (0..FRAME)
				.map(|sample| {
					((index * FRAME + sample) as f32 * 1_000.0 * std::f32::consts::TAU / 48_000.0).sin()
						* 0.25
				})
				.collect();
			suppressor.process(suppression, &mut frame);
		}
	}
	let mut output = Vec::with_capacity(clip.noisy.len());
	for chunk in clip.noisy.as_chunks::<FRAME>().0 {
		let mut frame = chunk.to_vec();
		suppressor.process(suppression, &mut frame);
		output.extend(frame);
	}
	let lag = delay(&clip.clean, &output, MAX_LAG);
	let aligned = &output[lag..];
	let speech = clip.speech().start..clip.speech().end.min(aligned.len());
	let noise = clip.settled_noise();
	let windows: Vec<String> = aligned[..LEAD_IN]
		.chunks(9_600)
		.map(|window| format!("{:.0}", level_db(window)))
		.collect();
	println!("  after tone {after_tone}: lead-in windows {}", windows.join(" "));
	println!(
		"  {snr_db:>4} dB: delay {lag} samples, SI-SDR {:.1} -> {:.1} dB, noise-only {:.1} -> {:.1} dB, speech {:.1} -> {:.1} dB",
		si_sdr_db(&clip.clean[speech.clone()], &clip.noisy[speech.clone()]),
		si_sdr_db(&clip.clean[speech.clone()], &aligned[speech.clone()]),
		level_db(&clip.noisy[noise.clone()]),
		level_db(&aligned[noise]),
		level_db(&clip.clean[speech.clone()]),
		level_db(&aligned[speech])
	);
}

fn ms(duration: Duration) -> f64 {
	duration.as_secs_f64() * 1000.0
}
