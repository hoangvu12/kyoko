use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::clip::{Clip, MAX_CHANNELS, frames_to_duration};
use crate::error::{AudioError, Result};
use crate::opus;

pub const DEFAULT_MAX_SAMPLES: usize = 48 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Limits {
	pub max_samples: usize,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_samples: DEFAULT_MAX_SAMPLES,
		}
	}
}

pub fn decode<B>(bytes: B, hint: Option<&str>, limits: &Limits) -> Result<Clip>
where
	B: AsRef<[u8]> + Send + Sync + 'static,
{
	guarded(move || {
		if opus::is_ogg_opus(bytes.as_ref()) {
			opus::decode(bytes.as_ref(), limits)
		} else {
			general(bytes, hint, limits)
		}
	})
}

pub fn duration<B>(bytes: B, hint: Option<&str>) -> Option<Duration>
where
	B: AsRef<[u8]> + Send + Sync + 'static,
{
	guarded(move || {
		if opus::is_ogg_opus(bytes.as_ref()) {
			return Ok(opus::duration(bytes.as_ref()));
		}
		let format = open(bytes, hint)?;
		let track = format.default_track(TrackType::Audio);
		let rate = track
			.and_then(|track| track.codec_params.as_ref())
			.and_then(|params| params.audio())
			.and_then(|params| params.sample_rate);
		Ok(track
			.and_then(|track| track.num_frames)
			.zip(rate)
			.map(|(frames, rate)| frames_to_duration(frames as f64, rate)))
	})
	.ok()
	.flatten()
}

fn guarded<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
	catch_unwind(AssertUnwindSafe(work))
		.unwrap_or_else(|_| Err(AudioError::Decode("the decoder gave up on this file".to_owned())))
}

fn open<B>(bytes: B, hint: Option<&str>) -> Result<Box<dyn FormatReader>>
where
	B: AsRef<[u8]> + Send + Sync + 'static,
{
	let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
	let mut probe_hint = Hint::new();
	if let Some(extension) = hint {
		probe_hint.with_extension(extension);
	}
	symphonia::default::get_probe()
		.probe(
			&probe_hint,
			source,
			FormatOptions::default(),
			MetadataOptions::default(),
		)
		.map_err(|error| AudioError::Unsupported(error.to_string()))
}

fn general<B>(bytes: B, hint: Option<&str>, limits: &Limits) -> Result<Clip>
where
	B: AsRef<[u8]> + Send + Sync + 'static,
{
	let mut format = open(bytes, hint)?;
	let track = format
		.default_track(TrackType::Audio)
		.ok_or_else(|| AudioError::Unsupported("no audio track".to_owned()))?;
	let track_id = track.id;
	let params = track
		.codec_params
		.as_ref()
		.and_then(|params| params.audio())
		.ok_or_else(|| AudioError::Unsupported("no audio codec".to_owned()))?
		.clone();
	let mut decoder = symphonia::default::get_codecs()
		.make_audio_decoder(&params, &AudioDecoderOptions::default())
		.map_err(|error| AudioError::Unsupported(error.to_string()))?;
	let mut shape: Option<(u32, usize)> = None;
	let mut samples: Vec<i16> = Vec::new();
	let mut scratch: Vec<i16> = Vec::new();
	loop {
		let packet = match format.next_packet() {
			Ok(Some(packet)) => packet,
			Ok(None) => break,
			Err(_) if !samples.is_empty() => break,
			Err(error) => return Err(AudioError::Decode(error.to_string())),
		};
		if packet.track_id != track_id {
			continue;
		}
		let buffer = match decoder.decode(&packet) {
			Ok(buffer) => buffer,
			Err(SymphoniaError::DecodeError(_)) => continue,
			Err(_) => break,
		};
		let rate = buffer.spec().rate();
		let channels = buffer.spec().channels().count();
		if channels == 0 || *shape.get_or_insert((rate, channels)) != (rate, channels) {
			continue;
		}
		scratch.clear();
		buffer.copy_to_vec_interleaved(&mut scratch);
		append(&mut samples, &scratch, channels);
		if samples.len() > limits.max_samples {
			return Err(AudioError::TooLong);
		}
	}
	let Some((rate, channels)) = shape.filter(|_| !samples.is_empty()) else {
		return Err(AudioError::Decode("no audio decoded".to_owned()));
	};
	Clip::new(rate, kept_channels(channels), samples)
}

fn kept_channels(channels: usize) -> u16 {
	if channels == 1 { 1 } else { MAX_CHANNELS }
}

fn append(samples: &mut Vec<i16>, interleaved: &[i16], channels: usize) {
	if channels <= usize::from(MAX_CHANNELS) {
		samples.extend_from_slice(interleaved);
		return;
	}
	for frame in interleaved.chunks_exact(channels) {
		let (left, right) = frame
			.iter()
			.enumerate()
			.fold((0i32, 0i32), |(left, right), (index, sample)| {
				if index % 2 == 0 {
					(left + i32::from(*sample), right)
				} else {
					(left, right + i32::from(*sample))
				}
			});
		let sides = channels as i32;
		samples.push((left / ((sides + 1) / 2)) as i16);
		samples.push((right / (sides / 2)) as i16);
	}
}
