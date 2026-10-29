use thiserror::Error;

pub const AUTHENTICATION_FAILED: u16 = 4004;
pub const MESSAGE_TOO_BIG: u16 = 1009;

#[derive(Debug, Error)]
pub enum GatewayError {
	#[error("gateway rejected the session: {}", fatal_meaning(*.code))]
	Fatal { code: u16 },
	#[error("gateway url is not valid")]
	InvalidUrl,
	#[error("gateway url must use wss, or ws on a loopback host")]
	InsecureUrl,
	#[error("incoming gateway data exceeded the size cap")]
	OverSized,
	#[error("gateway zlib stream is corrupt")]
	Decompress,
	#[error("failed to encode an outgoing frame: {0}")]
	Encode(#[from] serde_json::Error),
	#[error("websocket error: {0}")]
	Transport(String),
}

impl GatewayError {
	pub fn close_code(&self) -> Option<u16> {
		match self {
			Self::Fatal { code } => Some(*code),
			Self::OverSized => Some(MESSAGE_TOO_BIG),
			_ => None,
		}
	}

	pub fn is_authentication_failure(&self) -> bool {
		self.close_code() == Some(AUTHENTICATION_FAILED)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SendError {
	#[error("gateway send queue is full")]
	Full,
	#[error("gateway session has stopped")]
	Closed,
	#[error("outgoing gateway payload is not valid")]
	Invalid,
}

pub const FATAL_CLOSE_CODES: [u16; 8] = [AUTHENTICATION_FAILED, 4010, 4011, 4012, 4013, 4014, 4015, 4016];

fn fatal_meaning(code: u16) -> &'static str {
	match code {
		AUTHENTICATION_FAILED => "authentication failed",
		4010 => "invalid shard",
		4011 => "sharding required",
		4012 => "invalid api version",
		4013 => "invalid intents",
		4014 => "disallowed intents",
		_ => "fatal close code",
	}
}
