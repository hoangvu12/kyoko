use protocol::Snowflake;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

pub const GATEWAY_VERSION: &str = "9";
pub const OPUS_PAYLOAD_TYPE: u8 = 120;
pub const H264_PAYLOAD_TYPE: u8 = 101;
pub const H264_RTX_PAYLOAD_TYPE: u8 = 102;
pub const CODEC_PRIORITY: u32 = 1000;
pub const DAVE_PROTOCOL_VERSION: u8 = 1;

pub mod opcode {
	pub const IDENTIFY: u8 = 0;
	pub const SELECT_PROTOCOL: u8 = 1;
	pub const READY: u8 = 2;
	pub const HEARTBEAT: u8 = 3;
	pub const SESSION_DESCRIPTION: u8 = 4;
	pub const SPEAKING: u8 = 5;
	pub const HEARTBEAT_ACK: u8 = 6;
	pub const RESUME: u8 = 7;
	pub const HELLO: u8 = 8;
	pub const RESUMED: u8 = 9;
	pub const CLIENTS_CONNECT: u8 = 11;
	pub const CLIENT_DISCONNECT: u8 = 13;
	pub const MEDIA_SINK_WANTS: u8 = 15;
}

pub mod close {
	pub const AUTHENTICATION_FAILED: u16 = 4004;
	pub const SESSION_INVALID: u16 = 4006;
	pub const SESSION_TIMEOUT: u16 = 4009;
	pub const SERVER_NOT_FOUND: u16 = 4011;
	pub const DISCONNECTED: u16 = 4014;
	pub const SERVER_CRASHED: u16 = 4015;
	pub const E2EE_REQUIRED: u16 = 4017;
	pub const RATE_LIMITED: u16 = 4021;
	pub const CALL_TERMINATED: u16 = 4022;
}

#[derive(Debug, Serialize)]
pub struct Outbound<T> {
	pub op: u8,
	pub d: T,
}

#[derive(Debug, Serialize)]
pub struct Identify<'a> {
	pub server_id: Snowflake,
	pub channel_id: Snowflake,
	pub user_id: Snowflake,
	pub session_id: &'a str,
	pub token: &'a str,
	pub video: bool,
	pub streams: [StreamOffer; 2],
	pub max_dave_protocol_version: u8,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct StreamOffer {
	#[serde(rename = "type")]
	pub kind: &'static str,
	pub rid: &'static str,
	pub quality: u8,
}

pub const CAMERA_LAYERS: [StreamOffer; 2] = [
	StreamOffer {
		kind: "video",
		rid: "100",
		quality: 100,
	},
	StreamOffer {
		kind: "video",
		rid: "50",
		quality: 50,
	},
];

#[derive(Debug, Serialize)]
pub struct Resume<'a> {
	pub server_id: Snowflake,
	pub channel_id: Snowflake,
	pub session_id: &'a str,
	pub token: &'a str,
	pub seq_ack: i64,
}

#[derive(Debug, Serialize)]
pub struct Heartbeat {
	pub t: u64,
	pub seq_ack: i64,
}

#[derive(Debug, Serialize)]
pub struct SelectProtocol<'a> {
	pub protocol: &'static str,
	pub data: ProtocolData<'a>,
	pub codecs: [Codec; 2],
	pub rtc_connection_id: &'a str,
	pub experiments: &'a [String],
}

#[derive(Debug, Serialize)]
pub struct ProtocolData<'a> {
	pub address: &'a str,
	pub port: u16,
	pub mode: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Codec {
	pub name: &'static str,
	#[serde(rename = "type")]
	pub kind: &'static str,
	pub priority: u32,
	pub payload_type: u8,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rtx_payload_type: Option<u8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub encode: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub decode: Option<bool>,
}

pub const CODECS: [Codec; 2] = [
	Codec {
		name: "opus",
		kind: "audio",
		priority: CODEC_PRIORITY,
		payload_type: OPUS_PAYLOAD_TYPE,
		rtx_payload_type: None,
		encode: None,
		decode: None,
	},
	Codec {
		name: "H264",
		kind: "video",
		priority: CODEC_PRIORITY,
		payload_type: H264_PAYLOAD_TYPE,
		rtx_payload_type: Some(H264_RTX_PAYLOAD_TYPE),
		encode: Some(true),
		decode: Some(true),
	},
];

#[derive(Debug, Serialize)]
pub struct SpeakingUpdate {
	pub speaking: u32,
	pub delay: u32,
	pub ssrc: u32,
}

#[derive(Debug, Serialize)]
pub struct SinkWants {
	pub any: u8,
}

#[derive(Debug, Deserialize)]
pub struct RawFrame {
	#[serde(default, deserialize_with = "lenient")]
	pub op: Option<u8>,
	#[serde(default)]
	pub d: Value,
	#[serde(default, deserialize_with = "lenient")]
	pub seq: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct Hello {
	pub heartbeat_interval: f64,
}

#[derive(Debug, Deserialize)]
pub struct Ready {
	pub ssrc: u32,
	pub ip: String,
	pub port: u16,
	#[serde(default, deserialize_with = "lenient_strings")]
	pub modes: Vec<String>,
	#[serde(default, deserialize_with = "lenient_strings")]
	pub experiments: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SessionDescription {
	pub mode: String,
	pub secret_key: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct Speaking {
	#[serde(default, deserialize_with = "lenient")]
	pub speaking: Option<u32>,
	#[serde(default, deserialize_with = "lenient")]
	pub ssrc: Option<u32>,
	#[serde(default, deserialize_with = "lenient")]
	pub user_id: Option<Snowflake>,
}

#[derive(Debug, Deserialize)]
pub struct ClientsConnect {
	#[serde(default, deserialize_with = "lenient_ids")]
	pub user_ids: Vec<Snowflake>,
}

#[derive(Debug, Deserialize)]
pub struct ClientDisconnect {
	pub user_id: Snowflake,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatAck {
	#[serde(default, deserialize_with = "lenient")]
	pub t: Option<u64>,
}

fn lenient<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
	deserializer: D,
) -> Result<Option<T>, D::Error> {
	let value = Value::deserialize(deserializer)?;
	Ok(serde_json::from_value(value).ok())
}

fn lenient_strings<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
	lenient_list(deserializer)
}

fn lenient_ids<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Snowflake>, D::Error> {
	lenient_list(deserializer)
}

fn lenient_list<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
	deserializer: D,
) -> Result<Vec<T>, D::Error> {
	let Value::Array(values) = Value::deserialize(deserializer)? else {
		return Ok(Vec::new());
	};
	Ok(values
		.into_iter()
		.filter_map(|value| serde_json::from_value(value).ok())
		.collect())
}
