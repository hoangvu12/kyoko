use std::hash::{DefaultHasher, Hash, Hasher};

use media::cdn::{self, ProxyFormat, ProxySize, Quality, StickerFormat};
use media::{ImageRequest, MediaFetcher, MediaFetcherBuilder, Origin, Shape, SizeHint, Variant};
use protocol::fingerprint::Fingerprint;
use protocol::{Attachment, Embed, EmbedMedia, Member, Snowflake, StickerItem, User};
use session::Command;
use state::State;
use url::Url;

const ANIMATED_ATTACHMENT: u64 = 1 << 5;
const GIF: &str = "image/gif";
const WEBP: &str = "image/webp";
const ANIMATED_MEDIA: u64 = 1 << 5;
const TENOR_HOSTS: [&str; 2] = ["media.tenor.com", "c.tenor.com"];
const TENOR_FORMAT_CODE: usize = 5;
const TENOR_GIF_CODE: &str = "AAAAC";
const MOCK_MEDIA_EXTENSIONS: [&str; 4] = ["gif", "webp", "png", "json"];
const MOCK_MEDIA_FLAGS: [&str; 2] = ["animated", "passthrough"];
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn media_fetcher_builder(fingerprint: &Fingerprint) -> MediaFetcherBuilder {
	MediaFetcher::builder()
		.user_agent(fingerprint.user_agent())
		.headers(fingerprint.image_headers())
		.audio_headers(fingerprint.audio_headers())
}

pub fn media_fetcher(fingerprint: &Fingerprint, loopback_port: Option<u16>) -> media::Result<MediaFetcher> {
	let mut builder = media_fetcher_builder(fingerprint);
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
	Some(Avatar::of_user(user).request(cdn, points, scale, animate))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Avatar {
	pub user_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub hash: Option<String>,
	pub discriminator: Option<String>,
}

impl Avatar {
	pub fn of_user(user: &User) -> Self {
		Self {
			user_id: user.id,
			guild_id: None,
			hash: user.avatar.clone().filter(|hash| !hash.is_empty()),
			discriminator: user.discriminator.clone(),
		}
	}

	pub fn of_member(user: &User, guild_id: Option<Snowflake>, member: Option<&Member>) -> Self {
		let hash = member
			.and_then(|member| member.avatar.clone())
			.filter(|hash| !hash.is_empty());
		match (guild_id, hash) {
			(Some(guild_id), Some(hash)) => Self {
				guild_id: Some(guild_id),
				hash: Some(hash),
				..Self::of_user(user)
			},
			_ => Self::of_user(user),
		}
	}

	pub fn of(state: &State, guild_id: Option<Snowflake>, user: &User) -> Self {
		Self::of_member(user, guild_id, guild_member(state, guild_id, user.id))
	}

	pub fn is_animated(&self) -> bool {
		self.hash.as_deref().is_some_and(cdn::is_animated)
	}

	pub fn url(&self, cdn_base: &str, pixels: u32, animate: bool) -> String {
		match (self.guild_id, self.hash.as_deref()) {
			(Some(guild_id), Some(hash)) => {
				cdn::guild_member_avatar_url(cdn_base, guild_id.0, self.user_id.0, hash, pixels, animate)
			}
			(None, Some(hash)) => cdn::avatar_url(cdn_base, self.user_id.0, hash, pixels, animate),
			(_, None) => cdn::default_avatar_url(cdn_base, self.user_id.0, self.discriminator.as_deref()),
		}
	}

	pub fn request(&self, cdn_base: &str, points: f64, scale: f64, animate: bool) -> ImageRequest {
		let pixels = device_pixels(points, scale);
		ImageRequest::new(
			self.url(cdn_base, pixels, animate),
			Variant::new(SizeHint::square(pixels), Shape::Circle),
		)
	}

	pub fn moving_request(&self, cdn_base: &str, points: f64, scale: f64) -> Option<ImageRequest> {
		self.is_animated()
			.then(|| self.request(cdn_base, points, scale, true))
	}
}

pub fn guild_member(state: &State, guild_id: Option<Snowflake>, user_id: Snowflake) -> Option<&Member> {
	let guild_id = guild_id?;
	state
		.guild(guild_id)
		.and_then(|guild| guild.members.get(&user_id))
		.or_else(|| state.guild_profile(user_id, guild_id)?.member.as_ref())
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

pub fn app_tile_request(
	cdn: &str,
	application_id: Snowflake,
	hash: &str,
	points: f64,
	scale: f64,
) -> ImageRequest {
	let pixels = device_pixels(points, scale);
	ImageRequest::new(
		cdn::app_icon_url(cdn, application_id.0, hash, pixels),
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

pub fn local_media(url: &str, mock_origin: Option<&str>) -> String {
	let mut flags: Vec<String> = Vec::new();
	let local = local(url, mock_origin, |parsed| {
		let format = parsed
			.query_pairs()
			.find(|(key, _)| key == "format")
			.map(|(_, value)| value.to_ascii_lowercase());
		let extension = format.or_else(|| {
			parsed
				.path()
				.rsplit_once('.')
				.map(|(_, extension)| extension.to_ascii_lowercase())
		});
		flags = parsed
			.query_pairs()
			.filter(|(key, value)| MOCK_MEDIA_FLAGS.contains(&key.as_ref()) && value == "true")
			.map(|(key, _)| format!("{key}=true"))
			.collect();
		extension
			.filter(|extension| MOCK_MEDIA_EXTENSIONS.contains(&extension.as_str()))
			.unwrap_or_else(|| "png".to_owned())
	});
	if flags.is_empty() || local == url {
		local
	} else {
		format!("{local}?{}", flags.join("&"))
	}
}

pub fn local_audio(url: &str, mock_origin: Option<&str>) -> String {
	local(url, mock_origin, |parsed| {
		parsed
			.path()
			.rsplit_once('.')
			.map(|(_, extension)| extension.to_ascii_lowercase())
			.filter(|extension| {
				!extension.is_empty() && extension.chars().all(|letter| letter.is_ascii_alphanumeric())
			})
			.unwrap_or_else(|| "ogg".to_owned())
	})
}

fn local(url: &str, mock_origin: Option<&str>, extension: impl FnOnce(&Url) -> String) -> String {
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
		"{}/media/{:016x}.{}",
		origin.trim_end_matches('/'),
		hasher.finish(),
		extension(&parsed)
	)
}

fn is_gif_url(url: &str, content_type: Option<&str>) -> bool {
	content_type == Some(GIF)
		|| Url::parse(url).is_ok_and(|parsed| parsed.path().to_ascii_lowercase().ends_with(".gif"))
}

fn is_webp_url(url: &str, content_type: Option<&str>) -> bool {
	content_type == Some(WEBP)
		|| Url::parse(url).is_ok_and(|parsed| parsed.path().to_ascii_lowercase().ends_with(".webp"))
}

fn flagged_animated(media: &EmbedMedia) -> bool {
	media.flags.unwrap_or(0) & ANIMATED_MEDIA != 0
}

fn surely_moving(media: &EmbedMedia) -> bool {
	let url = media
		.proxy_url
		.as_deref()
		.or(media.url.as_deref())
		.unwrap_or_default();
	flagged_animated(media) || is_gif_url(url, media.content_type.as_deref())
}

pub fn embed_request(media: &EmbedMedia, box_points: (f64, f64), scale: f64) -> Option<ImageRequest> {
	let url = media.proxy_url.as_deref().or(media.url.as_deref())?;
	let width = cdn::device_pixels(box_points.0, scale);
	let height = cdn::device_pixels(box_points.1, scale);
	let url = if cdn::is_media_proxy(url) {
		let content_type = media.content_type.as_deref();
		let gif = is_gif_url(url, content_type);
		cdn::proxy_sized(
			url,
			ProxySize {
				width,
				height,
				format: (!gif).then_some(ProxyFormat::Webp),
				animated: !gif && (flagged_animated(media) || is_webp_url(url, content_type)),
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

pub fn gifv_request(embed: &Embed, box_points: (f64, f64), scale: f64) -> Option<ImageRequest> {
	let still_media = embed.image.as_ref().or(embed.thumbnail.as_ref());
	let still = still_media.and_then(|media| embed_request(media, box_points, scale));
	if still_media.is_some_and(surely_moving) {
		return still;
	}
	let width = cdn::device_pixels(box_points.0, scale);
	let height = cdn::device_pixels(box_points.1, scale);
	let Some(motion) = embed
		.video
		.as_ref()
		.and_then(|video| gifv_motion_url(video, width, height))
	else {
		return still.or_else(|| {
			embed
				.video
				.as_ref()
				.and_then(|video| embed_request(video, box_points, scale))
		});
	};
	let request = ImageRequest::new(motion, Variant::new(SizeHint::new(width, height), Shape::Square));
	Some(match still {
		Some(still) => request.or_else(still.url),
		None => request,
	})
}

fn gifv_motion_url(video: &EmbedMedia, width: u32, height: u32) -> Option<String> {
	if let Some(gif) = video.url.as_deref().and_then(tenor_gif_url) {
		return Some(gif);
	}
	let proxied = video
		.proxy_url
		.as_deref()
		.filter(|url| cdn::is_media_proxy(url))?;
	Some(cdn::proxy_sized(
		proxied,
		ProxySize {
			width,
			height,
			format: Some(ProxyFormat::Webp),
			animated: true,
		},
	))
}

pub fn tenor_gif_url(video: &str) -> Option<String> {
	let mut parsed = Url::parse(video).ok()?;
	if !TENOR_HOSTS.contains(&parsed.host_str()?) || !crate::gif_picker::is_video_url(video) {
		return None;
	}
	let segments: Vec<&str> = parsed.path_segments()?.collect();
	let [id, file] = segments.as_slice() else {
		return None;
	};
	if id.len() <= TENOR_FORMAT_CODE || !id.is_ascii() {
		return None;
	}
	let stem = file.rsplit_once('.').map_or(*file, |(stem, _)| stem);
	let path = format!(
		"/{}{TENOR_GIF_CODE}/{stem}.gif",
		&id[..id.len() - TENOR_FORMAT_CODE]
	);
	parsed.set_path(&path);
	parsed.set_query(None);
	Some(parsed.into())
}

pub fn embed_icon_request(url: &str, points: f64, scale: f64) -> ImageRequest {
	let pixels = cdn::device_pixels(points, scale);
	ImageRequest::new(url, Variant::new(SizeHint::square(pixels), Shape::Circle))
}

pub fn gif_still_request(gif: &protocol::Gif, box_points: (f64, f64), scale: f64) -> Option<ImageRequest> {
	let still = crate::gif_picker::gif_still(gif);
	if still.is_empty() || crate::gif_picker::is_video_url(still) {
		return None;
	}
	let hint = SizeHint::new(
		cdn::device_pixels(box_points.0, scale),
		cdn::device_pixels(box_points.1, scale),
	);
	Some(ImageRequest::new(still, Variant::new(hint, Shape::Square)))
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
