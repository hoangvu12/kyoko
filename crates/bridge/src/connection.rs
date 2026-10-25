use std::time::{Duration, Instant};

use state::{ConnectionStatus, DisconnectReason};

pub const OFFLINE_AFTER: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
	Live,
	Connecting,
	Reconnecting { retry_at: Option<Instant> },
	Offline { retry_at: Option<Instant> },
	Stopped(String),
	SignedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
	Warning,
	Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
	pub text: String,
	pub tone: Tone,
	pub retry: bool,
}

impl Connection {
	pub fn name(&self) -> &'static str {
		match self {
			Connection::Live => "live",
			Connection::Connecting => "connecting",
			Connection::Reconnecting { .. } => "reconnecting",
			Connection::Offline { .. } => "offline",
			Connection::Stopped(_) => "stopped",
			Connection::SignedOut => "signed_out",
		}
	}

	pub fn banner(&self, now: Instant) -> Option<Banner> {
		let (text, tone, retry) = match self {
			Connection::Live | Connection::SignedOut => return None,
			Connection::Connecting => ("Connecting...".to_owned(), Tone::Warning, false),
			Connection::Reconnecting { retry_at } => (
				match seconds_left(*retry_at, now) {
					Some(seconds) => format!("Reconnecting in {seconds}s..."),
					None => "Reconnecting...".to_owned(),
				},
				Tone::Warning,
				true,
			),
			Connection::Offline { retry_at } => (
				match seconds_left(*retry_at, now) {
					Some(seconds) => format!("You're offline. Retrying in {seconds}s..."),
					None => "You're offline. Reconnecting...".to_owned(),
				},
				Tone::Danger,
				true,
			),
			Connection::Stopped(message) => (
				format!("Disconnected from Discord: {message}"),
				Tone::Danger,
				false,
			),
		};
		Some(Banner { text, tone, retry })
	}

	pub fn next_change(&self, now: Instant) -> Option<Instant> {
		let retry_at = match self {
			Connection::Reconnecting { retry_at } | Connection::Offline { retry_at } => (*retry_at)?,
			_ => return None,
		};
		let seconds = seconds_left(Some(retry_at), now)?;
		Some(retry_at - Duration::from_secs(seconds - 1))
	}
}

fn seconds_left(retry_at: Option<Instant>, now: Instant) -> Option<u64> {
	let left = retry_at?.checked_duration_since(now)?;
	let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
	(seconds > 0).then_some(seconds)
}

#[derive(Debug)]
pub struct ConnectionWatch {
	last: Option<ConnectionStatus>,
	failures: u32,
	live_once: bool,
	current: Connection,
}

impl Default for ConnectionWatch {
	fn default() -> Self {
		Self {
			last: None,
			failures: 0,
			live_once: false,
			current: Connection::Connecting,
		}
	}
}

impl ConnectionWatch {
	pub fn showing(current: Connection) -> Self {
		Self {
			live_once: true,
			current,
			..Self::default()
		}
	}

	pub fn current(&self) -> &Connection {
		&self.current
	}

	pub fn observe(&mut self, status: &ConnectionStatus, now: Instant) -> bool {
		if self.last.as_ref() == Some(status) {
			return false;
		}
		self.last = Some(status.clone());
		let next = match status {
			ConnectionStatus::Connected => {
				self.failures = 0;
				self.live_once = true;
				Connection::Live
			}
			ConnectionStatus::Connecting | ConnectionStatus::Resuming => self.waiting(None),
			ConnectionStatus::Reconnecting { attempt, retry_in } => {
				self.failures = attempt.saturating_sub(1);
				self.live_once = true;
				self.waiting(Some(now + *retry_in))
			}
			ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed) => Connection::SignedOut,
			ConnectionStatus::Disconnected(DisconnectReason::Rejected { message, .. }) => {
				Connection::Stopped(message.clone())
			}
		};
		let changed = next != self.current;
		self.current = next;
		changed
	}

	fn waiting(&self, retry_at: Option<Instant>) -> Connection {
		if !self.live_once {
			Connection::Connecting
		} else if self.failures >= OFFLINE_AFTER {
			Connection::Offline { retry_at }
		} else {
			Connection::Reconnecting { retry_at }
		}
	}
}
