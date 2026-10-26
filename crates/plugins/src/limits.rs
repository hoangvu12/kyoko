use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
	pub send_deadline: Duration,
	pub veto_deadline: Duration,
	pub name_deadline: Duration,
	pub time_deadline: Duration,
	pub rank_deadline: Duration,
	pub render_deadline: Duration,
	pub event_deadline: Duration,
	pub action_deadline: Duration,
	pub load_deadline: Duration,
	pub queue_grace: Duration,
	pub memory: usize,
	pub strikes: usize,
	pub strike_window: Duration,
	pub backoff: Duration,
	pub backoff_factor: u32,
	pub suspensions: u32,
	pub actions: usize,
	pub action_window: Duration,
	pub storage_bytes: usize,
	pub storage_keys: usize,
	pub event_queue: usize,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			send_deadline: Duration::from_millis(250),
			veto_deadline: Duration::from_millis(100),
			name_deadline: Duration::from_millis(50),
			time_deadline: Duration::from_millis(50),
			rank_deadline: Duration::from_millis(100),
			render_deadline: Duration::from_millis(50),
			event_deadline: Duration::from_millis(200),
			action_deadline: Duration::from_millis(500),
			load_deadline: Duration::from_secs(1),
			queue_grace: Duration::from_millis(250),
			memory: 16 * 1024 * 1024,
			strikes: 3,
			strike_window: Duration::from_secs(60),
			backoff: Duration::from_secs(30),
			backoff_factor: 4,
			suspensions: 3,
			actions: 5,
			action_window: Duration::from_secs(10),
			storage_bytes: 1024 * 1024,
			storage_keys: 1024,
			event_queue: 256,
		}
	}
}

impl Limits {
	pub fn backoff_after(&self, suspensions: u32) -> Duration {
		let factor = self
			.backoff_factor
			.max(1)
			.saturating_pow(suspensions.saturating_sub(1));
		self.backoff.saturating_mul(factor)
	}
}
