use cpal::traits::HostTrait;
use denoise::{NoiseSuppressor, Suppression};
use opus::{Application, Channels, Decoder, Encoder};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::text;

const RATE: u32 = 48_000;
const FRAME: usize = 960;
const FRAMES: usize = 10;

pub fn tone(frames: usize, channels: usize) -> Vec<f32> {
	(0..frames)
		.flat_map(|index| {
			let phase = index as f32 * 1_000.0 * std::f32::consts::TAU / RATE as f32;
			std::iter::repeat_n(phase.sin() * 0.25, channels)
		})
		.collect()
}

pub fn opus_round_trip() -> Result<String, String> {
	let mut encoder = Encoder::new(RATE, Channels::Mono, Application::Voip).map_err(text)?;
	let mut decoder = Decoder::new(RATE, Channels::Stereo).map_err(text)?;
	let input = tone(FRAME, 1);
	let mut packet = [0u8; 1275];
	let mut stereo = vec![0.0f32; FRAME * 2];
	let mut bytes = 0;
	let mut decoded = 0;
	for _ in 0..FRAMES {
		let length = encoder.encode_float(&input, &mut packet).map_err(text)?;
		bytes += length;
		decoded += decoder
			.decode_float(&packet[..length], &mut stereo, false)
			.map_err(text)?;
	}
	Ok(format!(
		"{FRAMES} frames of a 1 kHz tone took {bytes} bytes, {decoded} samples per channel came back, {}",
		opus::version()
	))
}

pub fn resample() -> Result<String, String> {
	let mut fft = Fft::<f32>::new(44_100, RATE as usize, 441, 2, FixedSync::Input).map_err(text)?;
	let frames_in = fft.input_frames_next();
	let input = tone(frames_in, 2);
	let mut output = vec![0.0f32; fft.output_frames_max() * 2];
	let source = InterleavedSlice::new(&input, 2, frames_in).map_err(text)?;
	let frames_out = fft.output_frames_next();
	let mut sink = InterleavedSlice::new_mut(&mut output, 2, frames_out).map_err(text)?;
	let (read, written) = fft.process_into_buffer(&source, &mut sink, None).map_err(text)?;
	Ok(format!("{read} frames at 44.1 kHz became {written} at 48 kHz"))
}

pub fn suppress_noise() -> Result<String, String> {
	denoise::load(Suppression::HighQuality).map_err(text)?;
	let mut suppressor = NoiseSuppressor::new();
	let mut frame = tone(FRAME, 1);
	let used = suppressor.process(Suppression::HighQuality, &mut frame);
	Ok(format!("one {FRAME}-sample frame through {used:?}"))
}

pub fn devices() -> Result<String, String> {
	let host = cpal::default_host();
	let inputs = host.input_devices().map_err(text)?.count();
	let outputs = host.output_devices().map_err(text)?.count();
	Ok(format!(
		"{} host, {inputs} inputs, {outputs} outputs",
		host.id().name()
	))
}
