use std::time::Duration;

use protocol::Token;
use protocol::gateway::{DispatchEvent, Ready, ReadySupplemental};

use crate::error::GatewayError;

#[derive(Debug)]
pub enum GatewayEvent {
	Connecting {
		attempt: u32,
		resuming: bool,
	},
	Hello {
		heartbeat_interval: Duration,
	},
	TokenRefreshed(Token),
	Ready(Box<Ready>),
	ReadySupplemental(Box<ReadySupplemental>),
	Resumed,
	Dispatch {
		seq: u64,
		event: DispatchEvent,
	},
	Disconnected {
		code: Option<u16>,
		reason: String,
		will_resume: bool,
		attempt: u32,
		retry_in: Duration,
	},
	Fatal(GatewayError),
	Latency {
		heartbeat_rtt: Duration,
	},
	DecodeFailure {
		event: String,
	},
}
