use std::time::Duration;

use protocol::{IdentifyProperties, Token};

use crate::outbox::SendBudget;

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

#[derive(Debug)]
pub struct GatewayConfig {
	pub token: Token,
	pub gateway_url: String,
	pub identify: IdentifyProperties,
	pub capabilities: u64,
	pub initial_status: String,
	pub large_threshold: u32,
	pub tuning: GatewayTuning,
}

impl GatewayConfig {
	pub const DEFAULT_GATEWAY_URL: &'static str = "wss://gateway.discord.gg";
	pub const DEFAULT_CAPABILITIES: u64 =
		1 | 2 | 4 | 8 | 16 | 32 | 64 | 128 | 256 | 512 | 1024 | 2048 | 4096 | 8192 | 16384;
	pub const DEFAULT_LARGE_THRESHOLD: u32 = 250;

	pub fn new(token: Token, identify: IdentifyProperties) -> Self {
		Self {
			token,
			gateway_url: Self::DEFAULT_GATEWAY_URL.to_owned(),
			identify,
			capabilities: Self::DEFAULT_CAPABILITIES,
			initial_status: "online".to_owned(),
			large_threshold: Self::DEFAULT_LARGE_THRESHOLD,
			tuning: GatewayTuning::default(),
		}
	}
}
