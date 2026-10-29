use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutocompleteReply {
	#[default]
	Answer,
	Stall,
	Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraPlan {
	pub tiles: usize,
	pub fps: u32,
	pub clip: Option<PathBuf>,
}

impl FromStr for CameraPlan {
	type Err = String;

	fn from_str(text: &str) -> Result<Self, Self::Err> {
		const USAGE: &str = "cameras: expected <tiles>:<fps>[:<clip>]";
		let mut parts = text.splitn(3, ':');
		let tiles: usize = parts
			.next()
			.and_then(|part| part.trim().parse().ok())
			.ok_or(USAGE)?;
		let fps: u32 = parts
			.next()
			.and_then(|part| part.trim().parse().ok())
			.ok_or(USAGE)?;
		let clip = parts
			.next()
			.map(str::trim)
			.filter(|path| !path.is_empty())
			.map(PathBuf::from);
		Ok(Self {
			tiles,
			fps: fps.max(1),
			clip,
		})
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionTiming {
	pub followup: Duration,
	pub window: Duration,
	pub hold_ping: bool,
}

impl Default for InteractionTiming {
	fn default() -> Self {
		Self {
			followup: Duration::from_secs(2),
			window: Duration::from_secs(3),
			hold_ping: false,
		}
	}
}

#[derive(Debug, Clone)]
pub struct MockConfig {
	pub port: Option<u16>,
	pub seed: u64,
	pub guilds: usize,
	pub channels_per_guild: usize,
	pub messages_per_channel: usize,
	pub members_per_guild: usize,
	pub token: String,
	pub alt_token: String,
	pub heartbeat_interval_ms: u64,
	pub remote_auth_heartbeat_ms: u64,
	pub remote_auth_timeout_ms: u64,
	pub chaos: bool,
	pub live_traffic: bool,
	pub nitro: bool,
	pub autocomplete_emoji: bool,
	pub frequent_commands: bool,
	pub more_apps: bool,
	pub crowded_expressions: bool,
	pub media_delay: Duration,
	pub autocomplete: AutocompleteReply,
	pub interactions: InteractionTiming,
	pub failing_modals: bool,
	pub record: bool,
	pub anchor: Option<DateTime<Utc>>,
	pub build_number: u64,
	pub bucket_limit: u32,
	pub bucket_window: Duration,
	pub cameras: Option<CameraPlan>,
	pub streams: usize,
	pub answer_calls: bool,
	pub ring: bool,
}

impl Default for MockConfig {
	fn default() -> Self {
		Self {
			port: None,
			seed: 1,
			guilds: 3,
			channels_per_guild: 8,
			messages_per_channel: 200,
			members_per_guild: 50,
			token: "mock-token".to_owned(),
			alt_token: "mock-alt-token".to_owned(),
			heartbeat_interval_ms: 41_250,
			remote_auth_heartbeat_ms: 41_250,
			remote_auth_timeout_ms: 120_000,
			chaos: false,
			live_traffic: false,
			nitro: true,
			autocomplete_emoji: false,
			frequent_commands: false,
			more_apps: false,
			crowded_expressions: false,
			media_delay: Duration::ZERO,
			autocomplete: AutocompleteReply::Answer,
			interactions: InteractionTiming::default(),
			failing_modals: false,
			record: false,
			anchor: None,
			build_number: 424_242,
			bucket_limit: 5,
			bucket_window: Duration::from_secs(2),
			cameras: None,
			streams: 0,
			answer_calls: false,
			ring: false,
		}
	}
}

impl MockConfig {
	pub const CROWDED_GUILDS: usize = 50;
	pub const CROWDED_MEDIA_DELAY: Duration = Duration::from_millis(120);

	pub fn crowded() -> Self {
		Self {
			guilds: Self::CROWDED_GUILDS,
			channels_per_guild: 2,
			messages_per_channel: 20,
			members_per_guild: 10,
			crowded_expressions: true,
			media_delay: Self::CROWDED_MEDIA_DELAY,
			..Self::default()
		}
	}
}
