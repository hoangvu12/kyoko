use chrono::{DateTime, Utc};
use media::cdn::{self, Quality};
use media::{ImageRequest, Shape, SizeHint, Variant};
use protocol::{Attachment, Embed, EmbedKind, EmbedMedia, Message, Snowflake};

use crate::images::{Avatar, attachment_request, is_image};
use crate::mosaic::mosaic_images;
use crate::timeline::MessageEntry;

pub const LIGHTBOX_SHARE: f32 = 0.88;
pub const VIEWER_ZOOM_STEP: f32 = 2.;
pub const VIEWER_MAX_ZOOM: f32 = 8.;
const WHEEL_ZOOM_RATE: f32 = 0.002;
const FALLBACK_SIDE: f32 = 600.;
const DEFAULT_FILE_NAME: &str = "image";

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

#[derive(Debug, Clone, PartialEq)]
pub struct ViewerAuthor {
	pub name: String,
	pub avatar: Avatar,
	pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewing {
	pub images: Vec<LightboxImage>,
	pub index: usize,
	pub author: Option<ViewerAuthor>,
	pub message: Option<(Snowflake, Snowflake)>,
}

impl Viewing {
	pub fn alone(image: LightboxImage) -> Self {
		Self {
			images: vec![image],
			index: 0,
			author: None,
			message: None,
		}
	}

	pub fn of(entry: &MessageEntry, image: LightboxImage) -> Self {
		let mut images = message_images(&entry.message);
		let index = match images.iter().position(|shown| *shown == image) {
			Some(index) => index,
			None => {
				images = vec![image];
				0
			}
		};
		Self {
			images,
			index,
			author: Some(ViewerAuthor {
				name: entry.author.clone(),
				avatar: entry.avatar.clone(),
				sent_at: entry.sent_at,
			}),
			message: Some((entry.message.channel_id, entry.id)),
		}
	}

	pub fn current(&self) -> &LightboxImage {
		&self.images[self.index.min(self.images.len().saturating_sub(1))]
	}

	pub fn stepped(&self, by: isize) -> Option<Self> {
		let count = self.images.len() as isize;
		if count < 2 {
			return None;
		}
		let index = (self.index as isize + by).rem_euclid(count) as usize;
		Some(Self {
			index,
			..self.clone()
		})
	}
}

pub fn message_images(message: &Message) -> Vec<LightboxImage> {
	let attachments = mosaic_images(&message.attachments)
		.into_iter()
		.filter_map(|attachment| LightboxImage::attachment(attachment, message.channel_id, message.id));
	let embeds = message
		.embeds
		.iter()
		.filter_map(viewer_media)
		.filter_map(LightboxImage::embed);
	attachments.chain(embeds).collect()
}

fn viewer_media(embed: &Embed) -> Option<&EmbedMedia> {
	if matches!(embed.kind, Some(EmbedKind::Image | EmbedKind::Gifv)) {
		return embed
			.image
			.as_ref()
			.or(embed.thumbnail.as_ref())
			.or(embed.video.as_ref());
	}
	embed.image.as_ref().filter(|_| embed.video.is_none())
}

pub fn viewer_file_name(image: &LightboxImage) -> String {
	if !image.name.is_empty() {
		return image.name.clone();
	}
	image
		.original_url()
		.and_then(|url| url::Url::parse(url).ok())
		.and_then(|url| {
			url.path_segments()
				.and_then(|mut segments| segments.next_back().map(str::to_owned))
		})
		.filter(|name| !name.is_empty())
		.unwrap_or_else(|| DEFAULT_FILE_NAME.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
	pub scale: f32,
	pub pan: (f32, f32),
}

impl Default for Zoom {
	fn default() -> Self {
		Self::FIT
	}
}

impl Zoom {
	pub const FIT: Zoom = Zoom {
		scale: 1.,
		pan: (0., 0.),
	};

	pub fn zoomed(&self) -> bool {
		self.scale > 1.
	}

	pub fn toggled(self, fitted: (f32, f32), room: (f32, f32)) -> Self {
		if self.zoomed() {
			Self::FIT
		} else {
			self.scaled_to(VIEWER_ZOOM_STEP, (0., 0.), fitted, room)
		}
	}

	pub fn wheeled(self, delta_y: f32, about: (f32, f32), fitted: (f32, f32), room: (f32, f32)) -> Self {
		self.scaled_to(
			self.scale * (-delta_y * WHEEL_ZOOM_RATE).exp(),
			about,
			fitted,
			room,
		)
	}

	pub fn scaled_to(self, scale: f32, about: (f32, f32), fitted: (f32, f32), room: (f32, f32)) -> Self {
		let scale = scale.clamp(1., VIEWER_MAX_ZOOM);
		let grown = scale / self.scale;
		let pan = (
			about.0 - (about.0 - self.pan.0) * grown,
			about.1 - (about.1 - self.pan.1) * grown,
		);
		Self { scale, pan }.clamped(fitted, room)
	}

	pub fn panned(self, by: (f32, f32), fitted: (f32, f32), room: (f32, f32)) -> Self {
		Self {
			pan: (self.pan.0 + by.0, self.pan.1 + by.1),
			..self
		}
		.clamped(fitted, room)
	}

	pub fn size(&self, fitted: (f32, f32)) -> (f32, f32) {
		(fitted.0 * self.scale, fitted.1 * self.scale)
	}

	fn clamped(self, fitted: (f32, f32), room: (f32, f32)) -> Self {
		let (width, height) = self.size(fitted);
		let reach = (((width - room.0) / 2.).max(0.), ((height - room.1) / 2.).max(0.));
		Self {
			scale: self.scale,
			pan: (
				self.pan.0.clamp(-reach.0, reach.0),
				self.pan.1.clamp(-reach.1, reach.1),
			),
		}
	}
}
