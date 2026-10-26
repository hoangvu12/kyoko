use std::hash::{DefaultHasher, Hash, Hasher};

use media::cdn::{self, ProxyFormat, ProxySize, Quality, StickerFormat};
use media::{ImageRequest, MediaFetcher, Origin, Shape, SizeHint, Variant};
use protocol::fingerprint::Fingerprint;
use protocol::{Attachment, EmbedMedia, Snowflake, StickerItem, User};
use session::Command;
use url::Url;

const ANIMATED_ATTACHMENT: u64 = 1 << 5;
const GIF: &str = "image/gif";
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn media_fetcher(fingerprint: &Fingerprint, loopback_port: Option<u16>) -> media::Result<MediaFetcher> {
	let mut builder = MediaFetcher::builder()
		.user_agent(fingerprint.user_agent())
		.headers(fingerprint.image_headers());
	if let Some(port) = loopback_port {
		builder = builder.only_loopback(port);
	}
	builder.build()
}

pub fn device_pixels(points: f64, scale: f64) -> u32 {
	cdn::bucket(points, scale)
}

pub fn avatar_url(cdn: &str, user: &User, pixels: u32) -> Option<String> {
	let hash = user.avatar.as_deref()?;
	Some(cdn::still_avatar_url(cdn, user.id.0, hash, pixels))
}

pub fn avatar_request(
	cdn: &str,
	user: &User,
	points: f64,
	scale: f64,
	animate: bool,
) -> Option<ImageRequest> {
	let pixels = device_pixels(points, scale);
	let url = match user.avatar.as_deref() {
		Some(hash) => cdn::avatar_url(cdn, user.id.0, hash, pixels, animate),
		None => cdn::default_avatar_url(cdn, user.id.0, user.discriminator.as_deref()),
	};
	Some(ImageRequest::new(
		url,
		Variant::new(SizeHint::square(pixels), Shape::Circle),
	))
}

pub fn guild_icon_request(
	cdn: &str,
	guild_id: Snowflake,
	hash: &str,
	points: f64,
	scale: f64,
	animate: bool,
) -> ImageRequest {
	let pixels = device_pixels(points, scale);
	ImageRequest::new(
		cdn::guild_icon_url(cdn, guild_id.0, hash, pixels, animate),
		Variant::new(SizeHint::square(pixels), Shape::Square),
	)
}

pub fn app_icon_request(
	cdn: &str,
	application_id: Snowflake,
	hash: &str,
	points: f64,
	scale: f64,
) -> ImageRequest {
	let pixels = device_pixels(points, scale);
	ImageRequest::new(
		cdn::app_icon_url(cdn, application_id.0, hash, pixels),
		Variant::new(SizeHint::square(pixels), Shape::Circle),
	)
}

pub fn emoji_request(
	cdn: &str,
	emoji_id: Snowflake,
	animated: bool,
	points: f64,
	scale: f64,
) -> ImageRequest {
	let pixels = device_pixels(points, scale);
	ImageRequest::new(
		cdn::emoji_url(cdn, emoji_id.0, animated, pixels),
		Variant::new(SizeHint::square(pixels), Shape::Square),
	)
}

pub fn is_image(attachment: &Attachment) -> bool {
	match attachment.content_type.as_deref() {
		Some(content_type) => content_type.starts_with("image/"),
		None => attachment
			.filename
			.rsplit_once('.')
			.is_some_and(|(_, extension)| {
				IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
			}),
	}
}

fn is_gif(attachment: &Attachment) -> bool {
	attachment.content_type.as_deref() == Some(GIF)
		|| attachment.filename.to_ascii_lowercase().ends_with(".gif")
}

fn dimension(value: Option<i64>) -> u32 {
	value.and_then(|value| u32::try_from(value).ok()).unwrap_or(0)
}

pub fn attachment_request(
	attachment: &Attachment,
	channel_id: Snowflake,
	message_id: Snowflake,
	max_points: (f64, f64),
	scale: f64,
	quality: Quality,
) -> Option<ImageRequest> {
	if !is_image(attachment) {
		return None;
	}
	let (width, height) = cdn::fit(
		dimension(attachment.width),
		dimension(attachment.height),
		cdn::device_pixels(max_points.0, scale),
		cdn::device_pixels(max_points.1, scale),
	);
	let url = match quality {
		Quality::Original => attachment.url.as_ref().or(attachment.proxy_url.as_ref())?.clone(),
		Quality::Fitted => {
			let base = attachment.proxy_url.as_ref().or(attachment.url.as_ref())?;
			let gif = is_gif(attachment);
			cdn::proxy_sized(
				base,
				ProxySize {
					width,
					height,
					format: (!gif).then_some(ProxyFormat::Webp),
					animated: !gif && attachment.flags.unwrap_or(0) & ANIMATED_ATTACHMENT != 0,
				},
			)
		}
	};
	let origin = Origin {
		channel_id: channel_id.0,
		message_id: message_id.0,
	};
	Some(
		ImageRequest::new(url, Variant::new(SizeHint::new(width, height), Shape::Square))
			.from_message(origin),
	)
}

pub fn refresh_command(origin: Origin) -> Command {
	Command::RefreshMedia {
		channel_id: Snowflake(origin.channel_id),
		message_id: Snowflake(origin.message_id),
	}
}

pub fn local_media(url: &str, mock_origin: Option<&str>) -> String {
	let Some(origin) = mock_origin else {
		return url.to_owned();
	};
	let (Ok(parsed), Ok(home)) = (Url::parse(url), Url::parse(origin)) else {
		return url.to_owned();
	};
	if parsed.host_str() == home.host_str() {
		return url.to_owned();
	}
	let mut hasher = DefaultHasher::new();
	parsed.path().hash(&mut hasher);
	format!(
		"{}/media/{:016x}.png",
		origin.trim_end_matches('/'),
		hasher.finish()
	)
}

fn is_gif_url(url: &str, content_type: Option<&str>) -> bool {
	content_type == Some(GIF)
		|| Url::parse(url).is_ok_and(|parsed| parsed.path().to_ascii_lowercase().ends_with(".gif"))
}

pub fn embed_request(media: &EmbedMedia, box_points: (f64, f64), scale: f64) -> Option<ImageRequest> {
	let url = media.proxy_url.as_deref().or(media.url.as_deref())?;
	let width = cdn::device_pixels(box_points.0, scale);
	let height = cdn::device_pixels(box_points.1, scale);
	let url = if cdn::is_media_proxy(url) {
		let gif = is_gif_url(url, media.content_type.as_deref());
		cdn::proxy_sized(
			url,
			ProxySize {
				width,
				height,
				format: (!gif).then_some(ProxyFormat::Webp),
				animated: false,
			},
		)
	} else {
		url.to_owned()
	};
	Some(ImageRequest::new(
		url,
		Variant::new(SizeHint::new(width, height), Shape::Square),
	))
}

pub fn embed_icon_request(url: &str, points: f64, scale: f64) -> ImageRequest {
	let pixels = cdn::device_pixels(points, scale);
	ImageRequest::new(url, Variant::new(SizeHint::square(pixels), Shape::Circle))
}

pub fn sticker_request(sticker: &StickerItem, points: f64, scale: f64) -> Option<ImageRequest> {
	let format = StickerFormat::from(i64::from(sticker.format_type?));
	let url = cdn::sticker_url(sticker.id.0, format)?;
	let pixels = cdn::device_pixels(points, scale);
	Some(ImageRequest::new(
		url,
		Variant::new(SizeHint::square(pixels), Shape::Square),
	))
}

pub fn is_video(attachment: &Attachment) -> bool {
	attachment
		.content_type
		.as_deref()
		.is_some_and(|kind| kind.starts_with("video/"))
}
