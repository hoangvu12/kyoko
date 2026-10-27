use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::metadata::{LoopCount, Orientation};
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader};

use crate::cache::SizeHint;
use crate::error::{MediaError, Result};
use crate::lottie;
use crate::types::{Animation, Decoded, DecodedImage, Frame, Replay, Streamed};

const MIN_FRAME_DELAY_MS: u32 = 20;
const DEFAULT_FRAME_DELAY_MS: u32 = 100;
const ALLOC_FACTOR: u64 = 8;

#[derive(Debug, Clone)]
pub struct Limits {
	pub max_pixels: u64,
	pub max_frames: u32,
	pub max_bytes: usize,
	pub max_total_frame_pixels: u64,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_pixels: 32 * 1024 * 1024,
			max_frames: 500,
			max_bytes: 64 * 1024 * 1024,
			max_total_frame_pixels: 64 * 1024 * 1024,
		}
	}
}

pub type Prepare = Arc<dyn Fn(DecodedImage) -> DecodedImage + Send + Sync>;

#[derive(Clone)]
pub struct Plan {
	pub hint: SizeHint,
	pub kept_bytes: usize,
	pub prepare: Prepare,
}

impl Plan {
	pub fn keep_all() -> Self {
		Self {
			hint: SizeHint::ORIGINAL,
			kept_bytes: usize::MAX,
			prepare: Arc::new(|image| image),
		}
	}
}

type RasterFrames = Box<dyn Iterator<Item = image::ImageResult<image::Frame>>>;

enum Opened {
	Still(DecodedImage),
	Moving {
		frames: RasterFrames,
		loops: Option<u32>,
		frame_pixels: u64,
	},
}

pub fn decode(bytes: &[u8], limits: &Limits) -> Result<Decoded> {
	decode_planned(Bytes::copy_from_slice(bytes), limits, &Plan::keep_all())
}

pub fn decode_planned(bytes: Bytes, limits: &Limits, plan: &Plan) -> Result<Decoded> {
	if bytes.len() > limits.max_bytes {
		return Err(MediaError::Limit(format!(
			"{} bytes exceeds {}",
			bytes.len(),
			limits.max_bytes
		)));
	}
	if lottie::looks_like(&bytes) {
		return lottie::decode(&bytes, limits, plan);
	}
	match open(&bytes, limits)? {
		Opened::Still(image) => Ok(Decoded::Still((plan.prepare)(image))),
		Opened::Moving {
			frames,
			loops,
			frame_pixels,
		} => gather(&bytes, frames, loops, frame_pixels, limits, plan),
	}
}

fn open(bytes: &Bytes, limits: &Limits) -> Result<Opened> {
	let format =
		image::guess_format(bytes).map_err(|_| MediaError::Decode("unrecognized image format".to_owned()))?;
	let input = Cursor::new(bytes.clone());
	match format {
		ImageFormat::Gif => {
			let mut decoder = GifDecoder::new(input)?;
			let dimensions = prepare(&mut decoder, limits)?;
			Ok(moving(decoder, dimensions))
		}
		ImageFormat::WebP => {
			let mut decoder = WebPDecoder::new(input)?;
			let dimensions = prepare(&mut decoder, limits)?;
			if decoder.has_animation() {
				Ok(moving(decoder, dimensions))
			} else {
				decode_still(decoder).map(Opened::Still)
			}
		}
		ImageFormat::Png => {
			let mut decoder = PngDecoder::new(input)?;
			let dimensions = prepare(&mut decoder, limits)?;
			if decoder.is_apng()? {
				Ok(moving(decoder.apng()?, dimensions))
			} else {
				decode_still(decoder).map(Opened::Still)
			}
		}
		ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Ico => {
			let mut decoder = ImageReader::with_format(input, format).into_decoder()?;
			prepare(&mut decoder, limits)?;
			decode_still(decoder).map(Opened::Still)
		}
		other => Err(MediaError::Decode(format!("unsupported format {other:?}"))),
	}
}

fn moving(decoder: impl AnimationDecoder<'static>, (width, height): (u32, u32)) -> Opened {
	let loops = match decoder.loop_count() {
		LoopCount::Infinite => None,
		LoopCount::Finite(count) => Some(count.get()),
	};
	Opened::Moving {
		frames: Box::new(decoder.into_frames()),
		loops,
		frame_pixels: u64::from(width) * u64::from(height),
	}
}

fn prepare(decoder: &mut impl ImageDecoder, limits: &Limits) -> Result<(u32, u32)> {
	let (width, height) = decoder.dimensions();
	let pixels = u64::from(width) * u64::from(height);
	if pixels == 0 || pixels > limits.max_pixels {
		return Err(MediaError::Limit(format!(
			"{width}x{height} exceeds {} pixels",
			limits.max_pixels
		)));
	}
	let mut decoder_limits = image::Limits::default();
	decoder_limits.max_alloc = Some(limits.max_pixels.saturating_mul(ALLOC_FACTOR));
	decoder.set_limits(decoder_limits)?;
	Ok((width, height))
}

fn decode_still(mut decoder: impl ImageDecoder) -> Result<DecodedImage> {
	let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
	let mut image = DynamicImage::from_decoder(decoder)?;
	image.apply_orientation(orientation);
	Ok(into_decoded(image.into_rgba8()))
}

fn gather(
	bytes: &Bytes,
	frames: RasterFrames,
	loops: Option<u32>,
	frame_pixels: u64,
	limits: &Limits,
	plan: &Plan,
) -> Result<Decoded> {
	let mut kept: Vec<Frame> = Vec::new();
	let mut kept_bytes = 0_usize;
	let mut delays: Vec<u32> = Vec::new();
	let mut first: Option<DecodedImage> = None;
	let mut streaming = false;
	for frame in frames {
		let frame = frame?;
		let next_total = (delays.len() as u64 + 1) * frame_pixels;
		if delays.len() >= limits.max_frames as usize || next_total > limits.max_total_frame_pixels {
			return Err(MediaError::Limit(format!(
				"animation exceeds {} frames or {} total pixels",
				limits.max_frames, limits.max_total_frame_pixels
			)));
		}
		let delay_ms = frame_delay_ms(frame.delay().into());
		delays.push(delay_ms);
		if streaming {
			continue;
		}
		let image = (plan.prepare)(into_decoded(frame.into_buffer()));
		if first.is_none() {
			first = Some(image.clone());
		}
		kept_bytes = kept_bytes.saturating_add(image.rgba.len());
		if kept_bytes > plan.kept_bytes {
			streaming = true;
			kept.clear();
			continue;
		}
		kept.push(Frame { image, delay_ms });
	}
	let Some(first) = first else {
		return Err(MediaError::Decode("animation has no frames".to_owned()));
	};
	if delays.len() == 1 {
		return Ok(Decoded::Still(first));
	}
	if !streaming {
		return Ok(Decoded::Animated(Animation { frames: kept, loops }));
	}
	Ok(Decoded::Streamed(Streamed {
		first,
		delays_ms: delays,
		loops,
		source_bytes: bytes.len(),
		replay: raster_replay(bytes.clone(), limits.clone(), plan.prepare.clone()),
	}))
}

fn raster_replay(bytes: Bytes, limits: Limits, prepare: Prepare) -> Replay {
	Replay::new(move |sink| {
		let Ok(Opened::Moving { frames, .. }) = open(&bytes, &limits) else {
			return;
		};
		for frame in frames.take(limits.max_frames as usize) {
			let Ok(frame) = frame else {
				return;
			};
			if !sink(prepare(into_decoded(frame.into_buffer()))) {
				return;
			}
		}
	})
}

fn frame_delay_ms(delay: Duration) -> u32 {
	let millis = u32::try_from(delay.as_millis()).unwrap_or(u32::MAX);
	if millis < MIN_FRAME_DELAY_MS {
		DEFAULT_FRAME_DELAY_MS
	} else {
		millis
	}
}

fn into_decoded(buffer: image::RgbaImage) -> DecodedImage {
	DecodedImage {
		width: buffer.width(),
		height: buffer.height(),
		rgba: buffer.into_raw(),
	}
}
