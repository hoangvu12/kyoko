use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use protocol::{Message, Snowflake};
pub use state::AlertKind;

const QUEUE_LIMIT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertPacing {
	pub burst: usize,
	pub window: Duration,
}

impl Default for AlertPacing {
	fn default() -> Self {
		Self {
			burst: 3,
			window: Duration::from_secs(10),
		}
	}
}

#[derive(Debug, Clone)]
pub struct Alert {
	pub kind: AlertKind,
	pub guild_id: Option<Snowflake>,
	pub message: Arc<Message>,
}

impl Alert {
	pub fn channel_id(&self) -> Snowflake {
		self.message.channel_id
	}
}

#[derive(Clone, Default)]
pub(crate) struct AlertQueue(Arc<Mutex<VecDeque<Alert>>>);

impl AlertQueue {
	pub(crate) fn push(&self, alert: Alert) {
		let mut queue = self.0.lock();
		if queue.len() >= QUEUE_LIMIT {
			queue.pop_front();
		}
		queue.push_back(alert);
	}

	pub(crate) fn take(&self) -> Vec<Alert> {
		self.0.lock().drain(..).collect()
	}
}

pub(crate) struct Pacing {
	limits: AlertPacing,
	shown: VecDeque<Instant>,
}

impl Pacing {
	pub(crate) fn new(limits: AlertPacing) -> Self {
		Self {
			limits,
			shown: VecDeque::new(),
		}
	}

	pub(crate) fn allow(&mut self, now: Instant) -> bool {
		while self
			.shown
			.front()
			.is_some_and(|shown| now.duration_since(*shown) >= self.limits.window)
		{
			self.shown.pop_front();
		}
		if self.shown.len() >= self.limits.burst {
			return false;
		}
		self.shown.push_back(now);
		true
	}

	pub(crate) fn release(&mut self, at: Instant) {
		if let Some(index) = self.shown.iter().position(|shown| *shown == at) {
			self.shown.remove(index);
		}
	}
}
