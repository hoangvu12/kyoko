use media::{Decoded, DecodedImage, MediaService, SizeHint};
use protocol::User;

pub fn device_pixels(points: f64, scale: f64) -> u32 {
	let scale = if scale.is_finite() && scale > 0. {
		scale
	} else {
		1.
	};
	media::cdn::clamp_size((points * scale).ceil() as u32)
}

pub fn avatar_url(cdn: &str, user: &User, pixels: u32) -> Option<String> {
	let hash = user.avatar.as_deref()?;
	Some(media::cdn::still_avatar_url(cdn, user.id.0, hash, pixels))
}

pub async fn load_avatar(media: &MediaService, url: &str, pixels: u32) -> media::Result<DecodedImage> {
	let decoded = media.load(url, SizeHint::new(pixels, pixels)).await?;
	Ok(match decoded.as_ref() {
		Decoded::Still(image) => image.clone(),
		Decoded::Animated(animation) => animation.frames[0].image.clone(),
	})
}
