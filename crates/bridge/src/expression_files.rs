use std::io::Cursor;

use image::AnimationDecoder as _;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use state::{EMOJI_MAX_BYTES, STICKER_MAX_BYTES};

const EMOJI_SIDE: u32 = 128;
const STICKER_SIDE: u32 = 320;
const ICON_SIDE: u32 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickedImage {
	Icon,
	Emoji,
	Sticker,
}

impl PickedImage {
	pub fn side(self) -> u32 {
		match self {
			PickedImage::Icon => ICON_SIDE,
			PickedImage::Emoji => EMOJI_SIDE,
			PickedImage::Sticker => STICKER_SIDE,
		}
	}

	fn keeps_animation(self, raw: &[u8]) -> bool {
		match self {
			PickedImage::Icon => false,
			PickedImage::Emoji => raw.len() <= EMOJI_MAX_BYTES && animated_gif(raw),
			PickedImage::Sticker => {
				raw.len() <= STICKER_MAX_BYTES && (animated_gif(raw) || animated_png(raw))
			}
		}
	}
}

pub fn prepared_image(raw: Vec<u8>, kind: PickedImage) -> Option<Vec<u8>> {
	if kind.keeps_animation(&raw) {
		return Some(raw);
	}
	fitted_png(&raw, kind.side())
}

fn fitted_png(raw: &[u8], side: u32) -> Option<Vec<u8>> {
	let image = image::load_from_memory(raw).ok()?;
	let fitted = if image.width() > side || image.height() > side {
		image.thumbnail(side, side)
	} else {
		image
	};
	let mut encoded = Cursor::new(Vec::new());
	fitted
		.to_rgba8()
		.write_to(&mut encoded, image::ImageFormat::Png)
		.ok()?;
	Some(encoded.into_inner())
}

fn animated_gif(bytes: &[u8]) -> bool {
	GifDecoder::new(Cursor::new(bytes)).is_ok_and(|decoder| decoder.into_frames().take(2).count() == 2)
}

fn animated_png(bytes: &[u8]) -> bool {
	PngDecoder::new(Cursor::new(bytes)).is_ok_and(|decoder| decoder.is_apng().unwrap_or(false))
}
