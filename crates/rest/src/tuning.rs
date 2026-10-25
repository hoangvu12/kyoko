use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestTuning {
	pub mutation_gap: Duration,
	pub send_gap: Duration,
	pub channel_send_gap: Duration,
	pub delete_gap: Duration,
	pub bulk_ack_gap: Duration,
	pub ack_delay: Duration,
	pub ack_jitter: Duration,
	pub ack_max_delay: Duration,
	pub forbidden_strikes: u32,
	pub forbidden_window: Duration,
	pub breaker_cool_down: Duration,
	pub max_retry_wait: Duration,
	pub backfill_gap: Duration,
	pub backfill_per_minute: u32,
	pub backfill_cool_down: Duration,
	pub settings_delay: Duration,
	pub settings_gap: Duration,
}

impl Default for RestTuning {
	fn default() -> Self {
		Self {
			mutation_gap: Duration::from_millis(250),
			send_gap: Duration::from_millis(500),
			channel_send_gap: Duration::from_secs(1),
			delete_gap: Duration::from_millis(1200),
			bulk_ack_gap: Duration::from_secs(2),
			ack_delay: Duration::from_millis(1500),
			ack_jitter: Duration::from_secs(1),
			ack_max_delay: Duration::from_secs(10),
			forbidden_strikes: 3,
			forbidden_window: Duration::from_secs(60),
			breaker_cool_down: Duration::from_secs(300),
			max_retry_wait: Duration::from_secs(60),
			backfill_gap: Duration::from_millis(500),
			backfill_per_minute: 30,
			backfill_cool_down: Duration::from_secs(300),
			settings_delay: Duration::from_secs(1),
			settings_gap: Duration::from_secs(10),
		}
	}
}
