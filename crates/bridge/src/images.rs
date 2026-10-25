use media::cdn::{self, ProxyFormat, ProxySize, Quality};
use media::{ImageRequest, MediaFetcher, Origin, Shape, SizeHint, Variant};
use protocol::fingerprint::Fingerprint;
use protocol::{Attachment, Snowflake, User};
use session::Command;

const ANIMATED_ATTACHMENT: u64 = 1 << 5;
const GIF: &str = "image/gif";
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn media_fetcher(fingerprint: &Fingerprint, loopback_port: Option<u16>) -> media::Result<MediaFetcher> {
	let mut builder = MediaFetcher::builder()
		.user_agent(fingerprint.user_agent())
		.headers(fingerprint.image_headers());
	if let Some(port) = loopback_port {
		builder = builder.allow_loopback_http(port);
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
