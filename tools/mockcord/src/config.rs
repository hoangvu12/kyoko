use std::time::Duration;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutocompleteReply {
	#[default]
	Answer,
	Stall,
	Fail,
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
	pub autocomplete: AutocompleteReply,
	pub interactions: InteractionTiming,
	pub record: bool,
	pub anchor: Option<DateTime<Utc>>,
	pub build_number: u64,
	pub bucket_limit: u32,
	pub bucket_window: Duration,
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
			autocomplete: AutocompleteReply::Answer,
			interactions: InteractionTiming::default(),
			record: false,
			anchor: None,
			build_number: 424_242,
			bucket_limit: 5,
			bucket_window: Duration::from_secs(2),
		}
	}
}
