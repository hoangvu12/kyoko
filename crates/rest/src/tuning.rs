use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestTuning {
	pub mutation_gap: Duration,
	pub send_gap: Duration,
	pub channel_send_gap: Duration,
	pub forbidden_strikes: u32,
	pub forbidden_window: Duration,
	pub breaker_cool_down: Duration,
	pub max_retry_wait: Duration,
}

impl Default for RestTuning {
	fn default() -> Self {
		Self {
			mutation_gap: Duration::from_millis(250),
			send_gap: Duration::from_millis(500),
			channel_send_gap: Duration::from_secs(1),
			forbidden_strikes: 3,
			forbidden_window: Duration::from_secs(60),
			breaker_cool_down: Duration::from_secs(300),
			max_retry_wait: Duration::from_secs(60),
		}
	}
}
