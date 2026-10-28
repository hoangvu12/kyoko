use axum::http::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

pub const CLIP: &[u8] = include_bytes!("../fixtures/video/clip.mp4");
pub const CLIP_FILENAME: &str = "clip.mp4";
pub const CLIP_WIDTH: i64 = 320;
pub const CLIP_HEIGHT: i64 = 180;
pub const CLIP_SECS: f64 = 3.;
pub const CLIP_FPS: u32 = 15;
pub const MP4: &str = "video/mp4";
const EXTENSIONS: [&str; 4] = ["mp4", "webm", "mov", "m4v"];

pub fn is_video(file: &str) -> bool {
	file.rsplit_once('.')
		.is_some_and(|(_, extension)| EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
}

pub fn respond(headers: &HeaderMap) -> Response {
	let total = CLIP.len() as u64;
	let asked = headers
		.get(RANGE)
		.and_then(|value| value.to_str().ok())
		.map(|value| range(value, total));
	match asked {
		None => (
			[
				(CONTENT_TYPE, MP4.to_owned()),
				(ACCEPT_RANGES, "bytes".to_owned()),
				(CONTENT_LENGTH, total.to_string()),
			],
			CLIP,
		)
			.into_response(),
		Some(Some((first, last))) => (
			StatusCode::PARTIAL_CONTENT,
			[
				(CONTENT_TYPE, MP4.to_owned()),
				(ACCEPT_RANGES, "bytes".to_owned()),
				(CONTENT_RANGE, format!("bytes {first}-{last}/{total}")),
				(CONTENT_LENGTH, (last - first + 1).to_string()),
			],
			&CLIP[first as usize..=last as usize],
		)
			.into_response(),
		Some(None) => (
			StatusCode::RANGE_NOT_SATISFIABLE,
			[(CONTENT_RANGE, format!("bytes */{total}"))],
		)
			.into_response(),
	}
}

fn range(value: &str, total: u64) -> Option<(u64, u64)> {
	let span = value.trim().strip_prefix("bytes=")?;
	let (first, last) = span.split_once('-')?;
	let first: u64 = first.trim().parse().ok()?;
	let last = match last.trim() {
		"" => total.checked_sub(1)?,
		last => last.parse::<u64>().ok()?.min(total.checked_sub(1)?),
	};
	(first <= last).then_some((first, last))
}
