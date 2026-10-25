use std::hash::{DefaultHasher, Hash, Hasher};

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{Delay, DynamicImage, ExtendedColorType, Frame, ImageEncoder, ImageError, Rgba, RgbaImage};
use serde::Deserialize;
use serde_json::Value;

pub const SIGNED_FOR_SECS: i64 = 24 * 60 * 60;
const SIGNING_SECRET: &str = "mockcord-cdn";
const ATTACHMENT_WIDTH: u32 = 640;
const ATTACHMENT_HEIGHT: u32 = 480;
const MAX_SIDE: u32 = 2048;
const AVATAR_DEFAULT_SIZE: u32 = 128;
const AVATAR_MIN_SIZE: u32 = 16;
const AVATAR_MAX_SIZE: u32 = 512;
pub const ANIMATION_FRAMES: usize = 4;
pub const FRAME_DELAY_MS: u32 = 100;
const JPEG_QUALITY: u8 = 85;
const VP8X_ANIMATION: u8 = 0x02;
const VP8X_ALPHA: u8 = 0x10;
const ANMF_NO_BLEND: u8 = 0x02;
const RIFF_HEADER: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
	Fresh,
	Expired,
	Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
	Png,
	Jpeg,
	Gif,
	Webp,
}

impl Kind {
	fn of(extension: &str) -> Option<Self> {
		match extension.to_ascii_lowercase().as_str() {
			"png" => Some(Self::Png),
			"jpg" | "jpeg" => Some(Self::Jpeg),
			"gif" => Some(Self::Gif),
			"webp" => Some(Self::Webp),
			_ => None,
		}
	}

	fn content_type(self) -> &'static str {
		match self {
			Self::Png => "image/png",
			Self::Jpeg => "image/jpeg",
			Self::Gif => "image/gif",
			Self::Webp => "image/webp",
		}
	}
}

fn seed_of(name: &str) -> u64 {
	let mut hasher = DefaultHasher::new();
	name.hash(&mut hasher);
	hasher.finish()
}

fn lerp(from: u8, to: u8, amount: f32) -> u8 {
	(f32::from(from) + (f32::from(to) - f32::from(from)) * amount) as u8
}

fn render(seed: u64, width: u32, height: u32, frame: usize) -> RgbaImage {
	let bytes = seed.rotate_left(frame as u32 * 8).to_le_bytes();
	let inner = [bytes[0], bytes[1], bytes[2]];
	let outer = [bytes[3], bytes[4], bytes[5]];
	let centre_x = width as f32 / 2.;
	let centre_y = height as f32 / 2.;
	let reach = centre_x.max(centre_y);
	RgbaImage::from_fn(width, height, |x, y| {
		let dx = x as f32 + 0.5 - centre_x;
		let dy = y as f32 + 0.5 - centre_y;
		let amount = ((dx * dx + dy * dy).sqrt() / reach).min(1.);
		let [red, green, blue] = [0, 1, 2].map(|channel| lerp(inner[channel], outer[channel], amount));
		Rgba([red, green, blue, u8::MAX])
	})
}

fn frames(seed: u64, width: u32, height: u32, animated: bool) -> Vec<RgbaImage> {
	let count = if animated { ANIMATION_FRAMES } else { 1 };
	(0..count)
		.map(|frame| render(seed, width, height, frame))
		.collect()
}

fn encode(kind: Kind, frames: Vec<RgbaImage>) -> Result<Vec<u8>, ImageError> {
	let mut encoded = Vec::new();
	let first = &frames[0];
	let (width, height) = first.dimensions();
	match kind {
		Kind::Png => PngEncoder::new(&mut encoded).write_image(
			first.as_raw(),
			width,
			height,
			ExtendedColorType::Rgba8,
		)?,
		Kind::Jpeg => {
			let rgb = DynamicImage::ImageRgba8(first.clone()).into_rgb8();
			JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY).write_image(
				rgb.as_raw(),
				width,
				height,
				ExtendedColorType::Rgb8,
			)?
		}
		Kind::Gif => {
			let mut encoder = GifEncoder::new(&mut encoded);
			encoder.set_repeat(Repeat::Infinite)?;
			encoder.encode_frames(frames.into_iter().map(|image| {
				Frame::from_parts(image, 0, 0, Delay::from_numer_denom_ms(FRAME_DELAY_MS, 1))
			}))?;
		}
		Kind::Webp if frames.len() == 1 => WebPEncoder::new_lossless(&mut encoded).write_image(
			first.as_raw(),
			width,
			height,
			ExtendedColorType::Rgba8,
		)?,
		Kind::Webp => encoded = animated_webp(&frames)?,
	}
	Ok(encoded)
}

fn chunk(out: &mut Vec<u8>, name: &[u8; 4], body: &[u8]) {
	out.extend_from_slice(name);
	out.extend_from_slice(&(body.len() as u32).to_le_bytes());
	out.extend_from_slice(body);
	if body.len() % 2 == 1 {
		out.push(0);
	}
}

fn u24(out: &mut Vec<u8>, value: u32) {
	out.extend_from_slice(&value.to_le_bytes()[..3]);
}

fn animated_webp(frames: &[RgbaImage]) -> Result<Vec<u8>, ImageError> {
	let (width, height) = frames[0].dimensions();
	let mut body = b"WEBP".to_vec();
	let mut header = vec![VP8X_ANIMATION | VP8X_ALPHA, 0, 0, 0];
	u24(&mut header, width - 1);
	u24(&mut header, height - 1);
	chunk(&mut body, b"VP8X", &header);
	chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0]);
	for image in frames {
		let mut still = Vec::new();
		WebPEncoder::new_lossless(&mut still).write_image(
			image.as_raw(),
			width,
			height,
			ExtendedColorType::Rgba8,
		)?;
		let mut frame = Vec::new();
		u24(&mut frame, 0);
		u24(&mut frame, 0);
		u24(&mut frame, width - 1);
		u24(&mut frame, height - 1);
		u24(&mut frame, FRAME_DELAY_MS);
		frame.push(ANMF_NO_BLEND);
		frame.extend_from_slice(&still[RIFF_HEADER..]);
		chunk(&mut body, b"ANMF", &frame);
	}
	let mut file = b"RIFF".to_vec();
	file.extend_from_slice(&(body.len() as u32).to_le_bytes());
	file.extend_from_slice(&body);
	Ok(file)
}

fn respond(kind: Kind, frames: Vec<RgbaImage>) -> Response {
	match encode(kind, frames) {
		Ok(encoded) => ([(CONTENT_TYPE, kind.content_type())], encoded).into_response(),
		Err(error) => {
			tracing::warn!(%error, "could not encode the mock image");
			StatusCode::INTERNAL_SERVER_ERROR.into_response()
		}
	}
}

fn split_file(file: &str) -> Option<(&str, Kind)> {
	let (stem, extension) = file.rsplit_once('.')?;
	Some((stem, Kind::of(extension)?))
}

#[derive(Deserialize)]
pub struct SizedQuery {
	size: Option<u32>,
	animated: Option<bool>,
}

fn sized(seed_name: &str, stem: &str, kind: Kind, query: &SizedQuery) -> Response {
	let side = query
		.size
		.unwrap_or(AVATAR_DEFAULT_SIZE)
		.clamp(AVATAR_MIN_SIZE, AVATAR_MAX_SIZE);
	let animated_hash = stem.starts_with("a_");
	let animated = match kind {
		Kind::Gif => animated_hash,
		Kind::Webp => animated_hash && query.animated.unwrap_or(false),
		Kind::Png | Kind::Jpeg => false,
	};
	respond(kind, frames(seed_of(seed_name), side, side, animated))
}

pub async fn avatar(
	Path((user_id, file)): Path<(String, String)>,
	Query(query): Query<SizedQuery>,
) -> Response {
	let Some((hash, kind)) = split_file(&file) else {
		return StatusCode::NOT_FOUND.into_response();
	};
	sized(&format!("{user_id}/{hash}"), hash, kind, &query)
}

pub async fn default_avatar(Path(file): Path<String>, Query(query): Query<SizedQuery>) -> Response {
	let Some((index, kind)) = split_file(&file) else {
		return StatusCode::NOT_FOUND.into_response();
	};
	sized(&format!("embed/{index}"), index, kind, &query)
}

pub async fn guild_icon(
	Path((guild_id, file)): Path<(String, String)>,
	Query(query): Query<SizedQuery>,
) -> Response {
	let Some((hash, kind)) = split_file(&file) else {
		return StatusCode::NOT_FOUND.into_response();
	};
	sized(&format!("icons/{guild_id}/{hash}"), hash, kind, &query)
}

pub async fn emoji(Path(file): Path<String>, Query(query): Query<SizedQuery>) -> Response {
	let Some((id, kind)) = split_file(&file) else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let stem = if query.animated.unwrap_or(false) || kind == Kind::Gif {
		format!("a_{id}")
	} else {
		id.to_owned()
	};
	let query = SizedQuery {
		animated: Some(true),
		..query
	};
	sized(&format!("emoji/{id}"), &stem, kind, &query)
}

#[derive(Deserialize)]
pub struct AttachmentQuery {
	ex: Option<String>,
	is: Option<String>,
	hm: Option<String>,
	width: Option<u32>,
	height: Option<u32>,
	format: Option<String>,
}

fn now_secs() -> i64 {
	Utc::now().timestamp()
}

fn sign(path: &str, expires: &str, issued: &str) -> String {
	let mut hasher = DefaultHasher::new();
	(SIGNING_SECRET, path, expires, issued).hash(&mut hasher);
	format!("{:016x}", hasher.finish())
}

pub fn attachment_path(channel_id: u64, attachment_id: u64, filename: &str) -> String {
	format!("/cdn/attachments/{channel_id}/{attachment_id}/{filename}")
}

pub fn signed_query(path: &str, signature: Signature) -> String {
	let now = now_secs();
	let issued = format!("{:08x}", now.max(0));
	let expires = match signature {
		Signature::Expired => now - 60,
		Signature::Fresh | Signature::Revoked => now + SIGNED_FOR_SECS,
	};
	let expires = format!("{:08x}", expires.max(0));
	let mut hm = sign(path, &expires, &issued);
	if signature == Signature::Revoked {
		hm = hm.chars().rev().collect();
	}
	format!("ex={expires}&is={issued}&hm={hm}")
}

fn valid(path: &str, query: &AttachmentQuery) -> bool {
	let (Some(expires), Some(issued), Some(hm)) = (&query.ex, &query.is, &query.hm) else {
		return false;
	};
	let fresh = i64::from_str_radix(expires, 16).is_ok_and(|expires| expires > now_secs());
	fresh && *hm == sign(path, expires, issued)
}

pub async fn attachment(
	Path((channel_id, attachment_id, file)): Path<(u64, u64, String)>,
	Query(query): Query<AttachmentQuery>,
) -> Response {
	let path = attachment_path(channel_id, attachment_id, &file);
	let Some((_, stored)) = split_file(&file) else {
		return StatusCode::NOT_FOUND.into_response();
	};
	if !valid(&path, &query) {
		return (StatusCode::NOT_FOUND, "This content is no longer available.").into_response();
	}
	let kind = match query.format.as_deref().and_then(Kind::of) {
		Some(Kind::Webp) if stored != Kind::Gif => Kind::Webp,
		_ => stored,
	};
	let width = query.width.unwrap_or(ATTACHMENT_WIDTH).clamp(1, MAX_SIDE);
	let height = query.height.unwrap_or(ATTACHMENT_HEIGHT).clamp(1, MAX_SIDE);
	respond(kind, frames(seed_of(&path), width, height, stored == Kind::Gif))
}

pub fn attachment_size() -> (u32, u32) {
	(ATTACHMENT_WIDTH, ATTACHMENT_HEIGHT)
}

fn resign_url(origin: &str, url: &mut Value) {
	let Some(text) = url.as_str() else {
		return;
	};
	let Some(rest) = text.strip_prefix(origin) else {
		return;
	};
	let path = rest.split('?').next().unwrap_or(rest);
	if !path.starts_with("/cdn/attachments/") {
		return;
	}
	*url = Value::String(format!("{origin}{path}?{}", signed_query(path, Signature::Fresh)));
}

pub fn resign(origin: &str, message: &mut Value) {
	if let Some(attachments) = message["attachments"].as_array_mut() {
		for attachment in attachments {
			resign_url(origin, &mut attachment["url"]);
			resign_url(origin, &mut attachment["proxy_url"]);
		}
	}
}
