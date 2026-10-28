use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

pub mod opcode {
	pub const CLIENTS_CONNECT: u8 = 11;
	pub const CLIENT_DISCONNECT: u8 = 13;
	pub const PREPARE_TRANSITION: u8 = 21;
	pub const EXECUTE_TRANSITION: u8 = 22;
	pub const TRANSITION_READY: u8 = 23;
	pub const PREPARE_EPOCH: u8 = 24;
	pub const MLS_EXTERNAL_SENDER: u8 = 25;
	pub const MLS_KEY_PACKAGE: u8 = 26;
	pub const MLS_PROPOSALS: u8 = 27;
	pub const MLS_COMMIT_WELCOME: u8 = 28;
	pub const MLS_ANNOUNCE_COMMIT: u8 = 29;
	pub const MLS_WELCOME: u8 = 30;
	pub const MLS_INVALID_COMMIT_WELCOME: u8 = 31;
	pub const FIRST: u8 = PREPARE_TRANSITION;
	pub const LAST: u8 = MLS_INVALID_COMMIT_WELCOME;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wire {
	Text(String),
	Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
	KeyPackage(Vec<u8>),
	CommitWelcome {
		commit: Vec<u8>,
		welcome: Option<Vec<u8>>,
	},
	TransitionReady(u16),
	InvalidCommitWelcome(u16),
}

impl Outgoing {
	pub fn encode(&self) -> Wire {
		match self {
			Self::KeyPackage(key_package) => binary(opcode::MLS_KEY_PACKAGE, &[key_package]),
			Self::CommitWelcome { commit, welcome } => binary(
				opcode::MLS_COMMIT_WELCOME,
				&[commit, welcome.as_deref().unwrap_or_default()],
			),
			Self::TransitionReady(transition_id) => text(opcode::TRANSITION_READY, *transition_id),
			Self::InvalidCommitWelcome(transition_id) => {
				text(opcode::MLS_INVALID_COMMIT_WELCOME, *transition_id)
			}
		}
	}
}

fn binary(op: u8, parts: &[&[u8]]) -> Wire {
	let mut bytes = vec![op];
	for part in parts {
		bytes.extend_from_slice(part);
	}
	Wire::Binary(bytes)
}

fn text(op: u8, transition_id: u16) -> Wire {
	Wire::Text(json!({ "op": op, "d": { "transition_id": transition_id } }).to_string())
}

#[derive(Debug, Deserialize)]
pub struct PrepareTransition {
	pub transition_id: u16,
	#[serde(default, deserialize_with = "lenient")]
	pub protocol_version: u8,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteTransition {
	pub transition_id: u16,
}

#[derive(Debug, Deserialize)]
pub struct PrepareEpoch {
	#[serde(default, deserialize_with = "lenient")]
	pub protocol_version: u8,
	#[serde(default, deserialize_with = "lenient")]
	pub epoch: u64,
}

fn lenient<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned + Default>(
	deserializer: D,
) -> Result<T, D::Error> {
	let value = Value::deserialize(deserializer)?;
	Ok(serde_json::from_value(value).unwrap_or_default())
}
