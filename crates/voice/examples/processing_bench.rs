use std::time::{Duration, Instant};

use denoise::testing::{level_db, noise, speech};
use voice::{FRAME_SAMPLES, Processing, Processor};

const PASSES: usize = 5;
const ECHO_DELAY: usize = 2_400;
const ECHO_GAIN: f32 = 0.5;
const NEAR_END_SCALE: f32 = 0.05;
const SETTLED: usize = 100;

struct Spent {
	render: Vec<Duration>,
	echo: Vec<Duration>,
	gain: Vec<Duration>,
}

fn main() {
	let far_end = speech();
	let near_end = noise(far_end.len(), 3);
	let processing = Processing {
		echo_cancellation: true,
		auto_gain_control: true,
	};
	let mut spent = Spent {
		render: Vec::new(),
		echo: Vec::new(),
		gain: Vec::new(),
	};
	let (mut echo_in, mut echo_out) = (Vec::new(), Vec::new());
	for pass in 0..PASSES {
		let mut processor = Processor::default();
		let echo: Vec<f32> = std::iter::repeat_n(0.0, ECHO_DELAY)
			.chain(far_end.iter().map(|sample| sample * ECHO_GAIN))
			.collect();
		let frames = far_end.as_chunks::<FRAME_SAMPLES>().0;
		for (index, played) in frames.iter().enumerate() {
			let stereo: Vec<f32> = played.iter().flat_map(|sample| [*sample, *sample]).collect();
			let started = Instant::now();
			processor.render(processing, &stereo);
			spent.render.push(started.elapsed() / 2);
			let span = index * FRAME_SAMPLES..(index + 1) * FRAME_SAMPLES;
			let heard: Vec<f32> = echo[span.clone()].to_vec();
			let mut frame: Vec<f32> = heard
				.iter()
				.zip(&near_end[span])
				.map(|(echo, near)| echo + near * NEAR_END_SCALE)
				.collect();
			let started = Instant::now();
			processor.cancel_echo(processing, 0, &mut frame);
			spent.echo.push(started.elapsed() / 2);
			if pass == 0 && index >= SETTLED {
				echo_in.extend(&heard);
				echo_out.extend(&frame);
			}
			let started = Instant::now();
			processor.control_gain(processing, &mut frame);
			spent.gain.push(started.elapsed() / 2);
		}
	}
	report("render (AEC3 far end)", spent.render.clone());
	report("high-pass + AEC3", spent.echo.clone());
	report("AGC2", spent.gain.clone());
	let total = spent
		.render
		.iter()
		.zip(&spent.echo)
		.zip(&spent.gain)
		.map(|((render, echo), gain)| *render + *echo + *gain)
		.collect();
	report("whole chain", total);
	println!(
		"echo plus room noise {:.1} dB in, {:.1} dB out after {SETTLED} frames",
		level_db(&echo_in),
		level_db(&echo_out)
	);
}

fn report(name: &str, mut chunks: Vec<Duration>) {
	chunks.sort();
	let mean = chunks.iter().sum::<Duration>() / chunks.len() as u32;
	let at = |share: f64| chunks[((chunks.len() - 1) as f64 * share) as usize];
	println!(
		"{name}: per 10 ms over {} chunks: mean {:.1} us, median {:.1} us, p99 {:.1} us, max {:.1} us",
		chunks.len(),
		us(mean),
		us(at(0.5)),
		us(at(0.99)),
		us(at(1.0))
	);
}

fn us(duration: Duration) -> f64 {
	duration.as_secs_f64() * 1_000_000.0
}
