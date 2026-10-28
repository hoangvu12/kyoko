use std::collections::HashMap;
use std::time::{Duration, Instant};

use protocol::Snowflake;

pub(crate) const HANGOVER: Duration = Duration::from_millis(400);

#[derive(Default)]
pub(crate) struct Talkers {
	heard: HashMap<Snowflake, Instant>,
}

impl Talkers {
	pub fn heard(&mut self, user_id: Snowflake, now: Instant) -> bool {
		self.heard.insert(user_id, now).is_none()
	}

	pub fn hushed(&mut self, user_id: Snowflake) -> bool {
		self.heard.remove(&user_id).is_some()
	}

	pub fn fallen_quiet(&mut self, now: Instant) -> Vec<Snowflake> {
		let quiet: Vec<Snowflake> = self
			.heard
			.iter()
			.filter(|(_, at)| now.duration_since(**at) >= HANGOVER)
			.map(|(user_id, _)| *user_id)
			.collect();
		for user_id in &quiet {
			self.heard.remove(user_id);
		}
		quiet
	}

	pub fn clear(&mut self) -> Vec<Snowflake> {
		self.heard.drain().map(|(user_id, _)| user_id).collect()
	}
}
