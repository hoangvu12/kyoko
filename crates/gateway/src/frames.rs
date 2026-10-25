use protocol::gateway::Opcode;
use protocol::{ClientState, IdentifyProperties, Token};
use serde::Serialize;
use serde_json::{Value, json};

use crate::config::GatewayConfig;
use crate::error::GatewayError;

#[derive(Serialize)]
struct Outgoing<'a, T: Serialize> {
	op: Opcode,
	d: &'a T,
}

#[derive(Serialize)]
struct IdentifyData<'a> {
	token: &'a Token,
	properties: IdentifyProperties,
	compress: bool,
	presence: Presence<'a>,
	capabilities: u64,
	client_state: ClientState,
}

#[derive(Serialize)]
struct Presence<'a> {
	activities: [Value; 0],
	status: &'a str,
	since: u64,
	afk: bool,
}

#[derive(Serialize)]
struct ResumeData<'a> {
	token: &'a Token,
	session_id: &'a str,
	seq: u64,
}

fn encode<T: Serialize>(opcode: Opcode, data: &T) -> Result<String, GatewayError> {
	Ok(serde_json::to_string(&Outgoing { op: opcode, d: data })?)
}

pub(crate) fn identify(config: &GatewayConfig) -> Result<String, GatewayError> {
	encode(
		Opcode::Identify,
		&IdentifyData {
			token: &config.token,
			properties: config.properties.current(),
			compress: false,
			presence: Presence {
				activities: [],
				status: &config.initial_status,
				since: 0,
				afk: false,
			},
			capabilities: config.capabilities,
			client_state: config.client_state.current(),
		},
	)
}

pub(crate) fn resume(token: &Token, session_id: &str, seq: u64) -> Result<String, GatewayError> {
	encode(
		Opcode::Resume,
		&ResumeData {
			token,
			session_id,
			seq,
		},
	)
}

pub(crate) fn heartbeat(seq: Option<u64>) -> Result<String, GatewayError> {
	encode(Opcode::Heartbeat, &seq)
}

pub(crate) fn command(opcode: Opcode, data: Value) -> String {
	json!({ "op": i64::from(opcode), "d": data }).to_string()
}
