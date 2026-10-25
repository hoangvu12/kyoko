use std::hash::{DefaultHasher, Hash, Hasher};

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};
use serde::Deserialize;

const WIDTH: u32 = 400;
const HEIGHT: u32 = 300;
const GRID_STEP: u32 = 50;
const GRID_LIFT: u8 = 28;
const AVATAR_DEFAULT_SIZE: u32 = 128;
const AVATAR_MIN_SIZE: u32 = 16;
const AVATAR_MAX_SIZE: u32 = 512;

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

pub async fn media(Path(name): Path<String>) -> impl IntoResponse {
	let image = render(seed_of(&name));
	let mut encoded = Vec::new();
	let written =
		PngEncoder::new(&mut encoded).write_image(image.as_raw(), WIDTH, HEIGHT, ExtendedColorType::Rgb8);
	if let Err(error) = written {
		tracing::warn!(%error, "could not encode the mock image");
	}
	([(CONTENT_TYPE, "image/png")], encoded)
}

#[derive(Deserialize)]
pub struct AvatarQuery {
	size: Option<u32>,
}

fn render_avatar(seed: u64, side: u32) -> RgbaImage {
	let bytes = seed.to_le_bytes();
	let inner = [bytes[0], bytes[1], bytes[2]];
	let outer = [bytes[3], bytes[4], bytes[5]];
	let centre = side as f32 / 2.;
	RgbaImage::from_fn(side, side, |x, y| {
		let dx = x as f32 + 0.5 - centre;
		let dy = y as f32 + 0.5 - centre;
		let amount = ((dx * dx + dy * dy).sqrt() / centre).min(1.);
		let [red, green, blue] = [0, 1, 2].map(|channel| lerp(inner[channel], outer[channel], amount));
		Rgba([red, green, blue, u8::MAX])
	})
}

pub async fn avatar(
	Path((user_id, file)): Path<(String, String)>,
	Query(query): Query<AvatarQuery>,
) -> Response {
	let Some(hash) = file.strip_suffix(".webp") else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let side = query
		.size
		.unwrap_or(AVATAR_DEFAULT_SIZE)
		.clamp(AVATAR_MIN_SIZE, AVATAR_MAX_SIZE);
	let image = render_avatar(seed_of(&format!("{user_id}/{hash}")), side);
	let mut encoded = Vec::new();
	let written = WebPEncoder::new_lossless(&mut encoded).write_image(
		image.as_raw(),
		side,
		side,
		ExtendedColorType::Rgba8,
	);
	if let Err(error) = written {
		tracing::warn!(%error, "could not encode the mock avatar");
		return StatusCode::INTERNAL_SERVER_ERROR.into_response();
	}
	([(CONTENT_TYPE, "image/webp")], encoded).into_response()
}
