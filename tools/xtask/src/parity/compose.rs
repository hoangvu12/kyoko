use image::{Rgba, RgbaImage};

use super::font::{self, GLYPH_HEIGHT};
use super::{App, Shot};

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 800;
const GAP: u32 = 16;
const HEADER: u32 = 40;
const LABEL_SCALE: u32 = 3;
const TITLE_SCALE: u32 = 6;
const DETAIL_SCALE: u32 = 3;
const DETAIL_LIMIT: usize = 60;
const DIFF_THRESHOLD: u8 = 24;
const BACKDROP: Rgba<u8> = Rgba([30, 31, 34, 255]);
const PLACEHOLDER: Rgba<u8> = Rgba([49, 51, 56, 255]);
const STRIPE: Rgba<u8> = Rgba([43, 45, 49, 255]);
const LABEL: Rgba<u8> = Rgba([219, 222, 225, 255]);
const MUTED: Rgba<u8> = Rgba([148, 155, 164, 255]);
const DANGER: Rgba<u8> = Rgba([242, 63, 66, 255]);

pub fn shot_image(app: App, screen: &str, shot: &Shot) -> RgbaImage {
	match shot {
		Shot::Captured(image) => image.clone(),
		Shot::NotBuilt => placeholder(&format!("{app}: not built yet"), screen, LABEL),
		Shot::Skipped => placeholder(&format!("{app}: skipped"), screen, MUTED),
		Shot::Failed(reason) => placeholder(&format!("{app}: capture failed"), reason, DANGER),
	}
}

fn placeholder(title: &str, detail: &str, colour: Rgba<u8>) -> RgbaImage {
	let mut image = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
		if (x + y) / 24 % 2 == 0 {
			PLACEHOLDER
		} else {
			STRIPE
		}
	});
	let detail: String = detail.chars().take(DETAIL_LIMIT).collect();
	let title_height = GLYPH_HEIGHT * TITLE_SCALE;
	let top = HEIGHT / 2 - title_height;
	centred(&mut image, title, top, TITLE_SCALE, colour);
	centred(&mut image, &detail, top + title_height * 2, DETAIL_SCALE, MUTED);
	image
}

fn centred(image: &mut RgbaImage, text: &str, y: u32, scale: u32, colour: Rgba<u8>) {
	let width = font::text_width(text, scale).min(image.width());
	font::draw(image, text, (image.width() - width) / 2, y, scale, colour);
}

pub fn pair(left: &RgbaImage, right: &RgbaImage) -> RgbaImage {
	let width = left.width() + GAP + right.width();
	let height = HEADER + left.height().max(right.height());
	let mut image = RgbaImage::from_pixel(width, height, BACKDROP);
	let label_top = (HEADER - GLYPH_HEIGHT * LABEL_SCALE) / 2;
	font::draw(&mut image, App::Oxidcord.name(), 0, label_top, LABEL_SCALE, LABEL);
	font::draw(
		&mut image,
		App::Kyoko.name(),
		left.width() + GAP,
		label_top,
		LABEL_SCALE,
		LABEL,
	);
	image::imageops::replace(&mut image, left, 0, i64::from(HEADER));
	image::imageops::replace(
		&mut image,
		right,
		i64::from(left.width() + GAP),
		i64::from(HEADER),
	);
	image
}

pub fn diff(left: &RgbaImage, right: &RgbaImage) -> Option<(RgbaImage, f64)> {
	if left.dimensions() != right.dimensions() {
		return None;
	}
	let mut differing = 0u64;
	let image = RgbaImage::from_fn(left.width(), left.height(), |x, y| {
		let (a, b) = (left.get_pixel(x, y), right.get_pixel(x, y));
		let distance = (0..3)
			.map(|channel| a[channel].abs_diff(b[channel]))
			.max()
			.unwrap_or(0);
		if distance > DIFF_THRESHOLD {
			differing += 1;
			Rgba([255, 255 - distance, 255 - distance, 255])
		} else {
			let grey = ((u16::from(a[0]) + u16::from(a[1]) + u16::from(a[2])) / 9) as u8;
			Rgba([grey, grey, grey, 255])
		}
	});
	let pixels = u64::from(left.width()) * u64::from(left.height());
	Some((image, differing as f64 / pixels.max(1) as f64))
}
