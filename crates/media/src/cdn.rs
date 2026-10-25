use url::Url;

pub const CDN: &str = "https://cdn.discordapp.com";
const MEDIA_PROXY: &str = "https://media.discordapp.net";
const CDN_HOST: &str = "cdn.discordapp.com";
const PROXY_HOSTS: [&str; 3] = [
	"media.discordapp.net",
	"images-ext-1.discordapp.net",
	"images-ext-2.discordapp.net",
];
const MIN_SIZE: u32 = 16;
const MAX_SIZE: u32 = 4096;
const STICKER_SIZE: u32 = 320;
const DEFAULT_AVATAR_BUCKETS: u64 = 6;
const LEGACY_DEFAULT_AVATAR_BUCKETS: u32 = 5;
const SNOWFLAKE_TIMESTAMP_SHIFT: u32 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickerFormat {
	Png,
	Apng,
	Lottie,
	Gif,
	Unknown(i64),
}

impl From<i64> for StickerFormat {
	fn from(value: i64) -> Self {
		match value {
			1 => Self::Png,
			2 => Self::Apng,
			3 => Self::Lottie,
			4 => Self::Gif,
			other => Self::Unknown(other),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
	#[default]
	Fitted,
	Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyFormat {
	Webp,
	Png,
	Jpeg,
}

impl ProxyFormat {
	fn as_str(self) -> &'static str {
		match self {
			Self::Webp => "webp",
			Self::Png => "png",
			Self::Jpeg => "jpeg",
		}
	}
}

pub fn clamp_size(size: u32) -> u32 {
	size.clamp(MIN_SIZE, MAX_SIZE).next_power_of_two()
}

pub fn device_pixels(points: f64, scale: f64) -> u32 {
	let scale = if scale.is_finite() && scale > 0. {
		scale
	} else {
		1.
	};
	let pixels = (points.max(0.) * scale).ceil();
	if pixels >= f64::from(u32::MAX) {
		u32::MAX
	} else {
		pixels as u32
	}
}

pub fn bucket(points: f64, scale: f64) -> u32 {
	clamp_size(device_pixels(points, scale))
}

pub fn is_animated(hash: &str) -> bool {
	hash.starts_with("a_")
}

fn webp(hash: &str, size: u32, animate: bool) -> String {
	let animated = if animate && is_animated(hash) {
		"&animated=true"
	} else {
		""
	};
	format!("{hash}.webp?size={}{animated}", clamp_size(size))
}

pub fn avatar_url(cdn: &str, user_id: u64, hash: &str, size: u32, animate: bool) -> String {
	format!(
		"{}/avatars/{user_id}/{}",
		cdn.trim_end_matches('/'),
		webp(hash, size, animate)
	)
}

pub fn still_avatar_url(cdn: &str, user_id: u64, hash: &str, size: u32) -> String {
	avatar_url(cdn, user_id, hash, size, false)
}

pub fn default_avatar_url(user_id: u64, legacy_discriminator: Option<&str>) -> String {
	let index = match legacy_discriminator.and_then(|value| value.parse::<u32>().ok()) {
		Some(discriminator) if discriminator != 0 => u64::from(discriminator % LEGACY_DEFAULT_AVATAR_BUCKETS),
		_ => (user_id >> SNOWFLAKE_TIMESTAMP_SHIFT) % DEFAULT_AVATAR_BUCKETS,
	};
	format!("{CDN}/embed/avatars/{index}.png")
}

pub fn guild_icon_url(cdn: &str, guild_id: u64, hash: &str, size: u32, animate: bool) -> String {
	format!(
		"{}/icons/{guild_id}/{}",
		cdn.trim_end_matches('/'),
		webp(hash, size, animate)
	)
}

pub fn emoji_url(cdn: &str, id: u64, animated: bool, size: u32) -> String {
	let animated = if animated { "&animated=true" } else { "" };
	format!(
		"{}/emojis/{id}.webp?size={}&quality=lossless{animated}",
		cdn.trim_end_matches('/'),
		clamp_size(size)
	)
}

pub fn sticker_url(id: u64, format: StickerFormat) -> Option<String> {
	match format {
		StickerFormat::Png | StickerFormat::Apng => {
			Some(format!("{CDN}/stickers/{id}.png?size={STICKER_SIZE}"))
		}
		StickerFormat::Gif => Some(format!("{MEDIA_PROXY}/stickers/{id}.gif?size={STICKER_SIZE}")),
		StickerFormat::Lottie | StickerFormat::Unknown(_) => None,
	}
}

pub fn banner_url(owner_id: u64, hash: &str, size: u32, animate: bool) -> String {
	format!("{CDN}/banners/{owner_id}/{}", webp(hash, size, animate))
}

pub fn role_icon_url(role_id: u64, hash: &str, size: u32) -> String {
	format!("{CDN}/role-icons/{role_id}/{hash}.webp?size={}", clamp_size(size))
}

pub fn avatar_decoration_url(asset: &str, size: u32) -> String {
	let passthrough = if is_animated(asset) {
		"&passthrough=true"
	} else {
		""
	};
	format!(
		"{CDN}/avatar-decoration-presets/{asset}.png?size={}{passthrough}",
		clamp_size(size)
	)
}

pub fn fit(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
	if width == 0 || height == 0 || max_width == 0 || max_height == 0 {
		return (max_width, max_height);
	}
	if width <= max_width && height <= max_height {
		return (width, height);
	}
	let scale = (f64::from(max_width) / f64::from(width)).min(f64::from(max_height) / f64::from(height));
	(
		((f64::from(width) * scale).round() as u32).clamp(1, max_width),
		((f64::from(height) * scale).round() as u32).clamp(1, max_height),
	)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxySize {
	pub width: u32,
	pub height: u32,
	pub format: Option<ProxyFormat>,
	pub animated: bool,
}

pub fn proxy_sized(url: &str, sized: ProxySize) -> String {
	let Ok(mut parsed) = Url::parse(url) else {
		return url.to_owned();
	};
	if parsed.host_str() == Some(CDN_HOST) && parsed.path().starts_with("/attachments/") {
		let _ = parsed.set_host(Some(PROXY_HOSTS[0]));
	}
	let kept: Vec<(String, String)> = parsed
		.query_pairs()
		.filter(|(key, _)| {
			!key.is_empty()
				&& !matches!(
					key.as_ref(),
					"width" | "height" | "format" | "quality" | "animated"
				)
		})
		.map(|(key, value)| (key.into_owned(), value.into_owned()))
		.collect();
	{
		let mut query = parsed.query_pairs_mut();
		query.clear().extend_pairs(kept);
		if let Some(format) = sized.format {
			query.append_pair("format", format.as_str());
			if format == ProxyFormat::Webp {
				query.append_pair("quality", "lossless");
			}
		}
		if sized.animated {
			query.append_pair("animated", "true");
		}
		if sized.width > 0 {
			query.append_pair("width", &sized.width.to_string());
		}
		if sized.height > 0 {
			query.append_pair("height", &sized.height.to_string());
		}
	}
	parsed.into()
}

pub fn is_media_proxy(url: &str) -> bool {
	Url::parse(url)
		.ok()
		.and_then(|parsed| parsed.host_str().map(|host| PROXY_HOSTS.contains(&host)))
		.unwrap_or(false)
}

pub fn guild_member_avatar_url(
	cdn: &str,
	guild_id: u64,
	user_id: u64,
	hash: &str,
	size: u32,
	animate: bool,
) -> String {
	format!(
		"{}/guilds/{guild_id}/users/{user_id}/avatars/{}",
		cdn.trim_end_matches('/'),
		webp(hash, size, animate)
	)
}

pub fn badge_icon_url(icon: &str) -> String {
	format!("{CDN}/badge-icons/{icon}.png")
}
