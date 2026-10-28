use parking_lot::Mutex;
use protocol::Snowflake;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

use crate::crypto::EncryptionMode;
use crate::wire::close;

const EVENT_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
	Left,
	Disconnected,
	E2eeRequired,
	CallEnded,
	RateLimited,
	AuthenticationFailed,
	ServerNotFound,
	Closed(u16),
	Unreachable,
}

impl StopReason {
	pub fn from_close(code: Option<u16>) -> Self {
		match code {
			Some(close::DISCONNECTED) => Self::Disconnected,
			Some(close::E2EE_REQUIRED) => Self::E2eeRequired,
			Some(close::CALL_TERMINATED) => Self::CallEnded,
			Some(close::RATE_LIMITED) => Self::RateLimited,
			Some(close::AUTHENTICATION_FAILED) => Self::AuthenticationFailed,
			Some(close::SERVER_NOT_FOUND) => Self::ServerNotFound,
			Some(code) => Self::Closed(code),
			None => Self::Unreachable,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
	Connected {
		ssrc: u32,
		mode: EncryptionMode,
	},
	Resumed,
	Speaking {
		user_id: Snowflake,
		speaking: bool,
	},
	UserJoined {
		user_id: Snowflake,
	},
	UserLeft {
		user_id: Snowflake,
	},
	VideoStarted {
		user_id: Snowflake,
	},
	VideoStopped {
		user_id: Snowflake,
	},
	Reconnecting {
		code: Option<u16>,
		resume: bool,
	},
	Stopped(StopReason),
	Dave {
		epoch: u64,
		version: u8,
		members: Vec<Snowflake>,
	},
	DaveFailure(String),
	CameraStopped,
	Stats {
		ping_ms: u32,
		packets_lost: u64,
		jitter_ms: f32,
	},
}

#[derive(Default)]
pub(crate) struct EventHub {
	subscribers: Mutex<Vec<mpsc::Sender<VoiceEvent>>>,
	connected: Mutex<Option<VoiceEvent>>,
}

impl EventHub {
	pub fn subscribe(&self) -> mpsc::Receiver<VoiceEvent> {
		let (sender, receiver) = mpsc::channel(EVENT_CAPACITY);
		if let Some(connected) = self.connected.lock().clone() {
			let _ = sender.try_send(connected);
		}
		self.subscribers.lock().push(sender);
		receiver
	}

	pub fn emit(&self, event: VoiceEvent) {
		match &event {
			VoiceEvent::Connected { .. } => *self.connected.lock() = Some(event.clone()),
			VoiceEvent::Stopped(_) => *self.connected.lock() = None,
			_ => {}
		}
		self.subscribers
			.lock()
			.retain(|subscriber| !matches!(subscriber.try_send(event.clone()), Err(TrySendError::Closed(_))));
	}

	pub fn ssrc(&self) -> Option<u32> {
		match *self.connected.lock() {
			Some(VoiceEvent::Connected { ssrc, .. }) => Some(ssrc),
			_ => None,
		}
	}
}
