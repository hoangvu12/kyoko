use media::cdn::{self, Quality};
use media::{ImageRequest, Shape, SizeHint, Variant};
use protocol::{Attachment, EmbedMedia, Snowflake};

use crate::images::{attachment_request, is_image};

pub const LIGHTBOX_SHARE: f32 = 0.88;
const FALLBACK_SIDE: f32 = 600.;

#[derive(Debug, Clone, PartialEq)]
pub enum LightboxSource {
	Attachment {
		attachment: Box<Attachment>,
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	Embed(Box<EmbedMedia>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightboxImage {
	pub source: LightboxSource,
	pub name: String,
	pub width: u32,
	pub height: u32,
}

fn dimension(value: Option<i64>) -> u32 {
	value.and_then(|value| u32::try_from(value).ok()).unwrap_or(0)
}

impl LightboxImage {
	pub fn attachment(attachment: &Attachment, channel_id: Snowflake, message_id: Snowflake) -> Option<Self> {
		if !is_image(attachment) || (attachment.url.is_none() && attachment.proxy_url.is_none()) {
			return None;
		}
		Some(Self {
			source: LightboxSource::Attachment {
				attachment: Box::new(attachment.clone()),
				channel_id,
				message_id,
			},
			name: attachment.filename.clone(),
			width: dimension(attachment.width),
			height: dimension(attachment.height),
		})
	}

	pub fn embed(media: &EmbedMedia) -> Option<Self> {
		media.proxy_url.as_ref().or(media.url.as_ref())?;
		Some(Self {
			source: LightboxSource::Embed(Box::new(media.clone())),
			name: String::new(),
			width: dimension(media.width),
			height: dimension(media.height),
		})
	}

	pub fn original_url(&self) -> Option<&str> {
		match &self.source {
			LightboxSource::Attachment { attachment, .. } => {
				attachment.url.as_deref().or(attachment.proxy_url.as_deref())
			}
			LightboxSource::Embed(media) => media.url.as_deref().or(media.proxy_url.as_deref()),
		}
	}
}

pub fn lightbox_box(image: &LightboxImage, window: (f32, f32)) -> (f32, f32) {
	let (width, height) = if image.width > 0 && image.height > 0 {
		(image.width as f32, image.height as f32)
	} else {
		(FALLBACK_SIDE, FALLBACK_SIDE)
	};
	let scale = (window.0 * LIGHTBOX_SHARE / width).min(window.1 * LIGHTBOX_SHARE / height);
	((width * scale).round().max(1.), (height * scale).round().max(1.))
}

pub fn lightbox_request(image: &LightboxImage, window: (f32, f32), scale: f64) -> Option<ImageRequest> {
	let window = (f64::from(window.0), f64::from(window.1));
	match &image.source {
		LightboxSource::Attachment {
			attachment,
			channel_id,
			message_id,
		} => attachment_request(
			attachment,
			*channel_id,
			*message_id,
			window,
			scale,
			Quality::Original,
		),
		LightboxSource::Embed(media) => {
			let url = media.proxy_url.as_deref().or(media.url.as_deref())?;
			let hint = cdn::fit(
				image.width,
				image.height,
				cdn::device_pixels(window.0, scale),
				cdn::device_pixels(window.1, scale),
			);
			Some(ImageRequest::new(
				url,
				Variant::new(SizeHint::new(hint.0, hint.1), Shape::Square),
			))
		}
	}
}
