use std::time::{Duration, Instant};

use voice::video::{H264Decoder, Picture};

const PASSES: usize = 10;

fn main() {
	let mut args = std::env::args().skip(1);
	let path = args.next().expect("clip path");
	let mode = args.next().unwrap_or_default();
	let clip = std::fs::read(path).expect("clip");
	let frames = mockcord::clip_frames(&clip);
	let mut decoded = Vec::new();
	let started = Instant::now();
	let passes = if mode == "decode" { PASSES } else { 1 };
	for _ in 0..passes {
		let mut decoder = H264Decoder::new().expect("decoder");
		for (index, frame) in frames.iter().enumerate() {
			if let Ok(Some(picture)) = decoder.decode(frame, Duration::from_millis(index as u64 * 33)) {
				decoded.push(picture);
			}
		}
	}
	let decode = started.elapsed();
	println!(
		"decode: {} frames, {:.2} ms wall per frame",
		decoded.len(),
		decode.as_secs_f64() * 1000. / decoded.len() as f64
	);
	if let Some((width, height)) = mode
		.split_once('x')
		.and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
	{
		let started = Instant::now();
		for _ in 0..PASSES {
			for frame in &decoded {
				let _ = Picture::from_nv12(frame, width, height).expect("picture");
			}
		}
		let took = started.elapsed();
		println!(
			"convert to {width}x{height}: {:.2} ms per frame",
			took.as_secs_f64() * 1000. / (decoded.len() * PASSES) as f64
		);
	}
}
