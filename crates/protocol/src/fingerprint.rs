use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::snowflake::Snowflake;

pub const CHROME_MAJOR: u32 = 152;
pub const FALLBACK_BUILD_NUMBER: u64 = 649389;
pub const API_VERSION: u32 = 9;
pub const WEB_ORIGIN: &str = "https://discord.com";
pub const APP_URL: &str = "https://discord.com/app";
pub const HOME_REFERER: &str = "https://discord.com/channels/@me";
pub const MAX_INSTALLATION_ID_LEN: usize = 128;

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
	pub client_app_state: String,
	pub client_heartbeat_session_id: String,
}

impl IdentifyProperties {
	pub fn user_agent(&self) -> &str {
		&self.browser_user_agent
	}
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
				client_app_state: "focused".to_owned(),
				client_heartbeat_session_id: uuid::Uuid::new_v4().to_string(),
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

	pub fn super_properties(&self) -> String {
		STANDARD.encode(serde_json::to_vec(&self.properties).unwrap_or_default())
	}

	pub fn accept_language(&self) -> String {
		accept_language(self.locale())
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
	format!(
		"\"Chromium\";v=\"{CHROME_MAJOR}\", \"Not/A)Brand\";v=\"99\", \"Google Chrome\";v=\"{CHROME_MAJOR}\""
	)
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
