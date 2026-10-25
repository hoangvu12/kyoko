use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use protocol::capability::{
	AUTH_TOKEN_REFRESH, DEDUPE_USER_OBJECTS, LAZY_USER_NOTES, MULTIPLE_GUILD_EXPERIMENT_POPULATIONS,
	NON_CHANNEL_READ_STATES, PRIORITIZED_READY_PAYLOAD, VERSIONED_READ_STATES, VERSIONED_USER_GUILD_SETTINGS,
};
use protocol::{ClientState, IdentifyProperties, Token};

use crate::outbox::SendBudget;

pub const ADVERTISED_CAPABILITIES: u64 = LAZY_USER_NOTES
	| VERSIONED_READ_STATES
	| VERSIONED_USER_GUILD_SETTINGS
	| DEDUPE_USER_OBJECTS
	| PRIORITIZED_READY_PAYLOAD
	| MULTIPLE_GUILD_EXPERIMENT_POPULATIONS
	| NON_CHANNEL_READ_STATES
	| AUTH_TOKEN_REFRESH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
	pub base: Duration,
	pub cap: Duration,
}

impl Default for Backoff {
	fn default() -> Self {
		Self {
			base: Duration::from_secs(1),
			cap: Duration::from_secs(60),
		}
	}
}

impl Backoff {
	pub fn delay(&self, failures: u32) -> Duration {
		if failures == 0 {
			return Duration::ZERO;
		}
		let doublings = failures.saturating_sub(1).min(16) as i32;
		let jitter = rand::random_range(0.75..=1.25);
		let delay = self.base.as_secs_f64() * 2f64.powi(doublings) * jitter;
		Duration::from_secs_f64(delay.min(self.cap.as_secs_f64()))
	}
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GatewayTuning {
	pub backoff: Backoff,
	pub send_budget: SendBudget,
}

#[derive(Clone)]
pub struct ClientStateSource(Arc<dyn Fn() -> ClientState + Send + Sync>);

impl ClientStateSource {
	pub fn new(current: impl Fn() -> ClientState + Send + Sync + 'static) -> Self {
		Self(Arc::new(current))
	}

	pub fn current(&self) -> ClientState {
		(self.0)()
	}
}

impl Default for ClientStateSource {
	fn default() -> Self {
		Self::new(ClientState::default)
	}
}

impl fmt::Debug for ClientStateSource {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_tuple("ClientStateSource")
			.field(&self.current())
			.finish()
	}
}

#[derive(Debug)]
pub struct GatewayConfig {
	pub token: Token,
	pub gateway_url: String,
	pub identify: IdentifyProperties,
	pub capabilities: u64,
	pub initial_status: String,
	pub large_threshold: u32,
	pub tuning: GatewayTuning,
	pub client_state: ClientStateSource,
}

impl GatewayConfig {
	pub const DEFAULT_GATEWAY_URL: &'static str = "wss://gateway.discord.gg";
	pub const DEFAULT_LARGE_THRESHOLD: u32 = 250;

	pub fn new(token: Token, identify: IdentifyProperties) -> Self {
		Self {
			token,
			gateway_url: Self::DEFAULT_GATEWAY_URL.to_owned(),
			identify,
			capabilities: ADVERTISED_CAPABILITIES,
			initial_status: "online".to_owned(),
			large_threshold: Self::DEFAULT_LARGE_THRESHOLD,
			tuning: GatewayTuning::default(),
			client_state: ClientStateSource::default(),
		}
	}
}
