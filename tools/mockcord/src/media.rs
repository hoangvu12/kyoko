use std::hash::{DefaultHasher, Hash, Hasher};

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, Rgb, RgbImage, RgbaImage};
use serde::Deserialize;
use serde_json::json;

use crate::cdn::{FRAME_DELAY_MS, MEDIA_FRAMES};

const WIDTH: u32 = 400;
const HEIGHT: u32 = 300;
const GRID_STEP: u32 = 50;
const GRID_LIFT: u8 = 28;
const LOTTIE_SIDE: u32 = 160;
const LOTTIE_FRAMES: u32 = 30;
const LOTTIE_RATE: u32 = 30;
const MILLIS_PER_SECOND: u16 = 1000;

#[derive(Deserialize)]
pub struct MediaQuery {
	animated: Option<bool>,
	passthrough: Option<bool>,
}

fn seed_of(name: &str) -> u64 {
	let mut hasher = DefaultHasher::new();
	name.hash(&mut hasher);
	hasher.finish()
}

fn lerp(from: u8, to: u8, amount: f32) -> u8 {
	(f32::from(from) + (f32::from(to) - f32::from(from)) * amount) as u8
}

fn render(seed: u64) -> RgbImage {
	let bytes = seed.to_le_bytes();
	let from = [bytes[0], bytes[1], bytes[2]];
	let to = [bytes[3], bytes[4], bytes[5]];
	let span = (WIDTH + HEIGHT) as f32;
	RgbImage::from_fn(WIDTH, HEIGHT, |x, y| {
		let amount = (x + y) as f32 / span;
		let on_grid = x % GRID_STEP == 0 || y % GRID_STEP == 0;
		let lift = if on_grid { GRID_LIFT } else { 0 };
		Rgb([0, 1, 2].map(|channel| lerp(from[channel], to[channel], amount).saturating_add(lift)))
	})
}

fn frames(seed: u64) -> Vec<RgbaImage> {
	(0..MEDIA_FRAMES)
		.map(|frame| {
			let seed = if frame == 0 {
				seed
			} else {
				seed.rotate_left(frame as u32 * 8)
			};
			DynamicImage::ImageRgb8(render(seed)).into_rgba8()
		})
		.collect()
}

fn still(seed: u64) -> Result<Vec<u8>, String> {
	let image = render(seed);
	let mut encoded = Vec::new();
	PngEncoder::new(&mut encoded)
		.write_image(image.as_raw(), WIDTH, HEIGHT, ExtendedColorType::Rgb8)
		.map_err(|error| error.to_string())?;
	Ok(encoded)
}

fn apng(frames: &[RgbaImage]) -> Result<Vec<u8>, String> {
	let mut encoded = Vec::new();
	{
		let mut encoder = png::Encoder::new(&mut encoded, WIDTH, HEIGHT);
		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);
		encoder
			.set_animated(frames.len() as u32, 0)
			.map_err(|error| error.to_string())?;
		encoder
			.set_frame_delay(FRAME_DELAY_MS as u16, MILLIS_PER_SECOND)
			.map_err(|error| error.to_string())?;
		let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
		for frame in frames {
			writer
				.write_image_data(frame.as_raw())
				.map_err(|error| error.to_string())?;
		}
		writer.finish().map_err(|error| error.to_string())?;
	}
	Ok(encoded)
}

pub fn lottie(seed: u64) -> serde_json::Value {
	let bytes = seed.to_le_bytes();
	let colour = |index: usize| f64::from(bytes[index]) / 255.;
	let side = f64::from(LOTTIE_SIDE);
	let centre = side / 2.;
	json!({
		"v": "5.7.4",
		"fr": LOTTIE_RATE,
		"ip": 0,
		"op": LOTTIE_FRAMES,
		"w": LOTTIE_SIDE,
		"h": LOTTIE_SIDE,
		"layers": [{
			"ty": 4,
			"ind": 1,
			"ip": 0,
			"op": LOTTIE_FRAMES,
			"st": 0,
			"ks": {
				"a": {"a": 0, "k": [0, 0]},
				"p": {"a": 0, "k": [centre, centre]},
				"s": {"a": 0, "k": [100, 100]},
				"o": {"a": 0, "k": 100},
				"r": {"a": 1, "k": [
					{"t": 0, "s": [0], "o": {"x": [0.3], "y": [0]}, "i": {"x": [0.7], "y": [1]}},
					{"t": LOTTIE_FRAMES, "s": [360]}
				]}
			},
			"shapes": [{
				"ty": "gr",
				"it": [
					{"ty": "rc", "p": {"a": 0, "k": [0, 0]}, "s": {"a": 0, "k": [side * 0.7, side * 0.3]}, "r": {"a": 0, "k": 12}},
					{"ty": "el", "p": {"a": 0, "k": [0, -side * 0.25]}, "s": {"a": 0, "k": [side * 0.3, side * 0.3]}},
					{"ty": "fl", "c": {"a": 0, "k": [colour(0), colour(1), colour(2), 1]}, "o": {"a": 0, "k": 100}, "r": 1},
					{"ty": "st", "c": {"a": 0, "k": [colour(3), colour(4), colour(5), 1]}, "o": {"a": 0, "k": 100}, "w": {"a": 0, "k": 6}, "lc": 2, "lj": 2},
					{"ty": "tr", "p": {"a": 0, "k": [0, 0]}, "a": {"a": 0, "k": [0, 0]}, "s": {"a": 0, "k": [100, 100]}, "r": {"a": 0, "k": 0}, "o": {"a": 0, "k": 100}}
				]
			}]
		}]
	})
}

fn respond(content_type: &'static str, encoded: Result<Vec<u8>, String>) -> Response {
	match encoded {
		Ok(encoded) => ([(CONTENT_TYPE, content_type)], encoded).into_response(),
		Err(error) => {
			tracing::warn!(%error, "could not encode the mock image");
			StatusCode::INTERNAL_SERVER_ERROR.into_response()
		}
	}
}

pub async fn media(Path(name): Path<String>, Query(query): Query<MediaQuery>) -> Response {
	if let Some(audio) = crate::audio::respond(&name) {
		return audio;
	}
	let (stem, extension) = name.rsplit_once('.').unwrap_or((name.as_str(), "png"));
	let seed = seed_of(&format!("{stem}.png"));
	match extension {
		"gif" => respond(
			"image/gif",
			crate::cdn::encode_gif(frames(seed)).map_err(|error| error.to_string()),
		),
		"webp" if query.animated == Some(true) => respond(
			"image/webp",
			crate::cdn::encode_webp(frames(seed)).map_err(|error| error.to_string()),
		),
		"png" if query.passthrough == Some(true) => respond("image/png", apng(&frames(seed))),
		"json" => respond(
			"application/json",
			serde_json::to_vec(&lottie(seed)).map_err(|error| error.to_string()),
		),
		_ => respond("image/png", still(seed)),
	}
}
