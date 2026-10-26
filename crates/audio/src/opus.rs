use std::io::Cursor;
use std::time::Duration;

use ogg::reading::PacketReader;
use opus_decoder::OpusDecoder;

use crate::clip::{Clip, MAX_CHANNELS, frames_to_duration};
use crate::decode::Limits;
use crate::error::{AudioError, Result};

const RATE: u32 = 48_000;
const MAX_FRAME: usize = 5_760;
const CAPTURE: &[u8] = b"OggS";
const HEAD: &[u8] = b"OpusHead";
const TAGS: &[u8] = b"OpusTags";
const PAGE_HEADER: usize = 27;
const SEGMENT_COUNT: usize = 26;
const GRANULE: std::ops::Range<usize> = 6..14;
const HEAD_LEN: usize = 19;
const NO_GRANULE: u64 = u64::MAX;

struct Head {
	channels: u16,
	pre_skip: usize,
	gain: f32,
}

pub fn is_ogg_opus(bytes: &[u8]) -> bool {
	let Some(&segments) = bytes.get(SEGMENT_COUNT) else {
		return false;
	};
	let payload = PAGE_HEADER + usize::from(segments);
	bytes.starts_with(CAPTURE) && bytes.get(payload..payload + HEAD.len()) == Some(HEAD)
}

pub fn decode(bytes: &[u8], limits: &Limits) -> Result<Clip> {
	let mut reader = PacketReader::new(Cursor::new(bytes));
	let head = loop {
		match reader.read_packet() {
			Ok(Some(packet)) if packet.data.starts_with(HEAD) => break head(&packet.data)?,
			Ok(Some(_)) => continue,
			Ok(None) | Err(_) => return Err(AudioError::Decode("no Opus header".to_owned())),
		}
	};
	let channels = usize::from(head.channels);
	let mut decoder = OpusDecoder::new(RATE, channels)
		.map_err(|error| AudioError::Decode(format!("the Opus decoder refused the stream: {error:?}")))?;
	let mut pcm = vec![0i16; MAX_FRAME * channels];
	let mut samples: Vec<i16> = Vec::new();
	let mut last_granule = None;
	let mut decoded = 0usize;
	while let Ok(Some(packet)) = reader.read_packet() {
		if packet.data.starts_with(TAGS) || packet.data.starts_with(HEAD) {
			continue;
		}
		if packet.absgp_page() != NO_GRANULE {
			last_granule = Some(packet.absgp_page());
		}
		let Ok(frames) = decoder.decode(&packet.data, &mut pcm, false) else {
			continue;
		};
		let frames = frames.min(MAX_FRAME);
		if samples.len() + frames * channels > limits.max_samples {
			return Err(AudioError::TooLong);
		}
		samples.extend_from_slice(&pcm[..frames * channels]);
		decoded += 1;
	}
	if decoded == 0 {
		return Err(AudioError::Decode("no Opus packet decoded".to_owned()));
	}
	let skip = (head.pre_skip * channels).min(samples.len());
	samples.drain(..skip);
	if let Some(granule) = last_granule.and_then(|granule| usize::try_from(granule).ok())
		&& granule > head.pre_skip
	{
		samples.truncate((granule - head.pre_skip) * channels);
	}
	if head.gain != 1. {
		for sample in &mut samples {
			*sample = (f32::from(*sample) * head.gain).clamp(-32_768., 32_767.) as i16;
		}
	}
	Clip::new(RATE, head.channels, samples)
}

pub fn duration(bytes: &[u8]) -> Option<Duration> {
	let head = bytes
		.windows(HEAD.len())
		.position(|window| window == HEAD)
		.and_then(|start| head(&bytes[start..]).ok())?;
	let start = bytes
		.windows(CAPTURE.len())
		.rposition(|window| window == CAPTURE)?;
	let granule = bytes.get(start + GRANULE.start..start + GRANULE.end)?;
	let granule = u64::from_le_bytes(granule.try_into().ok()?);
	if granule == NO_GRANULE {
		return None;
	}
	let frames = granule.checked_sub(head.pre_skip as u64)?;
	Some(frames_to_duration(frames as f64, RATE))
}

fn head(data: &[u8]) -> Result<Head> {
	if data.len() < HEAD_LEN {
		return Err(AudioError::Decode("short Opus header".to_owned()));
	}
	let channels = u16::from(data[9]);
	let mapping = data[18];
	if channels == 0 || channels > MAX_CHANNELS || mapping != 0 {
		return Err(AudioError::Unsupported(format!(
			"Opus with {channels} channels, mapping {mapping}"
		)));
	}
	let pre_skip = usize::from(u16::from_le_bytes([data[10], data[11]]));
	let gain = i16::from_le_bytes([data[16], data[17]]);
	Ok(Head {
		channels,
		pre_skip,
		gain: 10f32.powf(f32::from(gain) / (20. * 256.)),
	})
}
