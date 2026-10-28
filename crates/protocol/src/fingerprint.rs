use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::snowflake::Snowflake;

pub const CHROME_MAJOR: u32 = 156;
pub const FALLBACK_BUILD_NUMBER: u64 = 649389;
pub const API_VERSION: u32 = 9;
pub const WEB_ORIGIN: &str = "https://discord.com";
pub const APP_URL: &str = "https://discord.com/app";
pub const HOME_REFERER: &str = "https://discord.com/channels/@me";
pub const LOGIN_REFERER: &str = "https://discord.com/login";
pub const MAX_INSTALLATION_ID_LEN: usize = 128;
pub const ACCEPT_ENCODING: &str = "gzip, deflate, br, zstd";
pub const IMAGE_ACCEPT: &str = "image/jxl,image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";
pub const IMAGE_REFERER: &str = "https://discord.com/";
pub const AUDIO_ACCEPT: &str = "*/*";
pub const AUDIO_ACCEPT_ENCODING: &str = "identity;q=1, *;q=0";
pub const AUDIO_RANGE: &str = "bytes=0-";
pub const VIDEO_ACCEPT: &str = "*/*";
pub const VIDEO_ACCEPT_ENCODING: &str = "identity";
pub const HEARTBEAT_SESSION_LIFETIME: Duration = Duration::from_secs(30 * 60);
pub const LAUNCH_SIGNATURE_MOD_BITS: u128 = (1 << 119)
	| (1 << 108)
	| (1 << 100)
	| (1 << 91)
	| (1 << 84)
	| (1 << 75)
	| (1 << 61)
	| (1 << 55)
	| (1 << 48)
	| (1 << 38)
	| (1 << 24)
	| (1 << 11);

const GREASE_CHARACTERS: [char; 11] = [' ', '(', ':', '-', '.', '/', ')', ';', '=', '?', '_'];
const GREASE_VERSIONS: [&str; 3] = ["8", "99", "24"];
const BRAND_ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentifyProperties {
	pub os: String,
	pub browser: String,
	pub device: String,
	pub system_locale: String,
	pub browser_user_agent: String,
	pub browser_version: String,
	pub os_version: String,
	pub referrer: String,
	pub referring_domain: String,
	pub referrer_current: String,
	pub referring_domain_current: String,
	pub release_channel: String,
	pub client_build_number: u64,
	pub client_event_source: Value,
	pub has_client_mods: bool,
	pub client_launch_id: String,
	pub launch_signature: String,
	pub client_app_state: String,
	pub client_heartbeat_session_id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub installation_id: Option<String>,
}

impl IdentifyProperties {
	pub fn user_agent(&self) -> &str {
		&self.browser_user_agent
	}

	pub fn super_properties(&self) -> Self {
		Self {
			installation_id: None,
			..self.clone()
		}
	}
}

#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> Instant + Send + Sync>);

impl Clock {
	pub fn system() -> Self {
		Self::new(Instant::now)
	}

	pub fn new(now: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
		Self(Arc::new(now))
	}

	pub fn now(&self) -> Instant {
		(self.0)()
	}
}

impl Default for Clock {
	fn default() -> Self {
		Self::system()
	}
}

impl fmt::Debug for Clock {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str("Clock")
	}
}

pub fn heartbeat_session_expired(started: Instant, now: Instant) -> bool {
	now.saturating_duration_since(started) >= HEARTBEAT_SESSION_LIFETIME
}

pub fn launch_signature() -> String {
	uuid::Uuid::from_u128(uuid::Uuid::new_v4().as_u128() & !LAUNCH_SIGNATURE_MOD_BITS).to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
	properties: IdentifyProperties,
	timezone: Option<String>,
}

impl Fingerprint {
	pub fn web_client(locale: &str, build_number: u64) -> Self {
		Self {
			properties: IdentifyProperties {
				os: host_os().to_owned(),
				browser: "Chrome".to_owned(),
				device: String::new(),
				system_locale: locale.to_owned(),
				browser_user_agent: user_agent(),
				browser_version: browser_version(),
				os_version: host_os_version().to_owned(),
				referrer: String::new(),
				referring_domain: String::new(),
				referrer_current: String::new(),
				referring_domain_current: String::new(),
				release_channel: "stable".to_owned(),
				client_build_number: build_number,
				client_event_source: Value::Null,
				has_client_mods: false,
				client_launch_id: uuid::Uuid::new_v4().to_string(),
				launch_signature: launch_signature(),
				client_app_state: "focused".to_owned(),
				client_heartbeat_session_id: uuid::Uuid::new_v4().to_string(),
				installation_id: None,
			},
			timezone: iana_time_zone::get_timezone()
				.ok()
				.filter(|zone| plausible_header(zone)),
		}
	}

	pub fn properties(&self) -> &IdentifyProperties {
		&self.properties
	}

	pub fn user_agent(&self) -> &str {
		&self.properties.browser_user_agent
	}

	pub fn locale(&self) -> &str {
		&self.properties.system_locale
	}

	pub fn timezone(&self) -> Option<&str> {
		self.timezone.as_deref()
	}

	pub fn build_number(&self) -> u64 {
		self.properties.client_build_number
	}

	pub fn set_build_number(&mut self, build_number: u64) {
		self.properties.client_build_number = build_number;
	}

	pub fn heartbeat_session_id(&self) -> &str {
		&self.properties.client_heartbeat_session_id
	}

	pub fn renew_heartbeat_session(&mut self) {
		self.properties.client_heartbeat_session_id = uuid::Uuid::new_v4().to_string();
	}

	pub fn identify_properties(&self, installation_id: Option<&str>) -> IdentifyProperties {
		IdentifyProperties {
			installation_id: installation_id.map(str::to_owned),
			..self.properties.clone()
		}
	}

	pub fn super_properties(&self) -> String {
		STANDARD.encode(serde_json::to_vec(&self.properties.super_properties()).unwrap_or_default())
	}

	pub fn accept_language(&self) -> String {
		accept_language(self.locale())
	}

	pub fn image_headers(&self) -> Vec<(&'static str, String)> {
		vec![
			("accept", IMAGE_ACCEPT.to_owned()),
			("accept-encoding", ACCEPT_ENCODING.to_owned()),
			("accept-language", self.accept_language()),
			("priority", "i".to_owned()),
			("referer", IMAGE_REFERER.to_owned()),
			("sec-ch-ua", sec_ch_ua()),
			("sec-ch-ua-mobile", "?0".to_owned()),
			("sec-ch-ua-platform", sec_ch_ua_platform().to_owned()),
			("sec-fetch-dest", "image".to_owned()),
			("sec-fetch-mode", "no-cors".to_owned()),
			("sec-fetch-site", "cross-site".to_owned()),
			("sec-fetch-storage-access", "active".to_owned()),
		]
	}

	pub fn audio_headers(&self) -> Vec<(&'static str, String)> {
		vec![
			("accept", AUDIO_ACCEPT.to_owned()),
			("accept-encoding", AUDIO_ACCEPT_ENCODING.to_owned()),
			("accept-language", self.accept_language()),
			("range", AUDIO_RANGE.to_owned()),
			("referer", IMAGE_REFERER.to_owned()),
			("sec-ch-ua", sec_ch_ua()),
			("sec-ch-ua-mobile", "?0".to_owned()),
			("sec-ch-ua-platform", sec_ch_ua_platform().to_owned()),
			("sec-fetch-dest", "audio".to_owned()),
			("sec-fetch-mode", "no-cors".to_owned()),
			("sec-fetch-site", "cross-site".to_owned()),
		]
	}

	pub fn video_headers(&self) -> Vec<(&'static str, String)> {
		vec![
			("accept", VIDEO_ACCEPT.to_owned()),
			("accept-encoding", VIDEO_ACCEPT_ENCODING.to_owned()),
			("accept-language", self.accept_language()),
			("priority", "i".to_owned()),
			("referer", IMAGE_REFERER.to_owned()),
			("sec-ch-ua", sec_ch_ua()),
			("sec-ch-ua-mobile", "?0".to_owned()),
			("sec-ch-ua-platform", sec_ch_ua_platform().to_owned()),
			("sec-fetch-dest", "video".to_owned()),
			("sec-fetch-mode", "no-cors".to_owned()),
			("sec-fetch-site", "cross-site".to_owned()),
			("sec-fetch-storage-access", "active".to_owned()),
		]
	}
}

pub fn user_agent() -> String {
	format!(
		"Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{} Safari/537.36",
		ua_platform(),
		browser_version()
	)
}

pub fn browser_version() -> String {
	format!("{CHROME_MAJOR}.0.0.0")
}

pub fn accept_language(locale: &str) -> String {
	match locale.split('-').next() {
		Some(language) if language != locale => format!("{locale},{language};q=0.9"),
		_ => locale.to_owned(),
	}
}

pub fn sec_ch_ua() -> String {
	brand_list(CHROME_MAJOR)
		.iter()
		.map(|(brand, version)| format!("\"{brand}\";v=\"{version}\""))
		.collect::<Vec<_>>()
		.join(", ")
}

fn brand_list(major: u32) -> [(String, String); 3] {
	let seed = major as usize;
	let grease_brand = format!(
		"Not{}A{}Brand",
		GREASE_CHARACTERS[seed % GREASE_CHARACTERS.len()],
		GREASE_CHARACTERS[(seed + 1) % GREASE_CHARACTERS.len()]
	);
	let grease_version = GREASE_VERSIONS[seed % GREASE_VERSIONS.len()].to_owned();
	let generated = [
		(grease_brand, grease_version),
		("Chromium".to_owned(), major.to_string()),
		("Google Chrome".to_owned(), major.to_string()),
	];
	let order = BRAND_ORDERS[seed % BRAND_ORDERS.len()];
	let mut shuffled: [(String, String); 3] = Default::default();
	for (brand, position) in generated.into_iter().zip(order) {
		shuffled[position] = brand;
	}
	shuffled
}

pub fn sec_ch_ua_platform() -> &'static str {
	if cfg!(target_os = "macos") {
		"\"macOS\""
	} else if cfg!(target_os = "linux") {
		"\"Linux\""
	} else {
		"\"Windows\""
	}
}

pub fn channel_referer(guild_id: Option<Snowflake>, channel_id: Snowflake) -> String {
	match guild_id {
		Some(guild_id) => format!("{WEB_ORIGIN}/channels/{guild_id}/{channel_id}"),
		None => format!("{WEB_ORIGIN}/channels/@me/{channel_id}"),
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinLocation {
	JoinGuild,
	InviteButtonEmbed,
}

impl JoinLocation {
	pub fn label(self) -> &'static str {
		match self {
			Self::JoinGuild => "Join Guild",
			Self::InviteButtonEmbed => "Invite Button Embed",
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipLocation {
	AddFriend,
}

impl RelationshipLocation {
	pub fn label(self) -> &'static str {
		match self {
			Self::AddFriend => "Add Friend",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextPlace {
	pub location_guild_id: Option<Snowflake>,
	pub location_channel_id: Option<Snowflake>,
	pub location_channel_type: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextProperties {
	pub location: &'static str,
	#[serde(flatten, skip_serializing_if = "Option::is_none")]
	pub place: Option<ContextPlace>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub location_message_id: Option<Snowflake>,
}

impl ContextProperties {
	pub fn join(
		location: JoinLocation,
		guild_id: Option<Snowflake>,
		channel_id: Option<Snowflake>,
		channel_type: Option<i64>,
	) -> Self {
		Self {
			location: location.label(),
			place: Some(ContextPlace {
				location_guild_id: guild_id,
				location_channel_id: channel_id,
				location_channel_type: channel_type,
			}),
			location_message_id: None,
		}
	}

	pub fn relationship(location: RelationshipLocation) -> Self {
		Self {
			location: location.label(),
			place: None,
			location_message_id: None,
		}
	}

	pub fn with_message(mut self, message_id: Snowflake) -> Self {
		self.location_message_id = Some(message_id);
		self
	}

	pub fn header_value(&self) -> String {
		STANDARD.encode(serde_json::to_vec(self).unwrap_or_default())
	}
}

pub fn plausible_installation_id(id: &str) -> bool {
	!id.is_empty() && id.len() <= MAX_INSTALLATION_ID_LEN && plausible_header(id)
}

fn plausible_header(value: &str) -> bool {
	value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn host_os() -> &'static str {
	if cfg!(target_os = "macos") {
		"Mac OS X"
	} else if cfg!(target_os = "linux") {
		"Linux"
	} else {
		"Windows"
	}
}

fn host_os_version() -> &'static str {
	if cfg!(target_os = "macos") {
		"10.15.7"
	} else if cfg!(target_os = "linux") {
		""
	} else {
		"10"
	}
}

fn ua_platform() -> &'static str {
	if cfg!(target_os = "macos") {
		"Macintosh; Intel Mac OS X 10_15_7"
	} else if cfg!(target_os = "linux") {
		"X11; Linux x86_64"
	} else {
		"Windows NT 10.0; Win64; x64"
	}
}
