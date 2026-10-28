use std::time::Instant;

use serde_json::Value;

#[cfg(feature = "tls")]
use crate::hello::{ClientHello, Http1Request};
use crate::record::field_names;

#[derive(Debug, Clone)]
pub struct VoiceSocket {
	pub id: u64,
	#[cfg(feature = "tls")]
	pub hello: Option<ClientHello>,
	#[cfg(feature = "tls")]
	pub alpn: Option<String>,
	#[cfg(feature = "tls")]
	pub upgrade: Option<Http1Request>,
	pub at: Instant,
}

impl VoiceSocket {
	pub(crate) fn new(id: u64) -> Self {
		Self {
			id,
			#[cfg(feature = "tls")]
			hello: None,
			#[cfg(feature = "tls")]
			alpn: None,
			#[cfg(feature = "tls")]
			upgrade: None,
			at: Instant::now(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct VoiceFrame {
	pub connection: u64,
	pub op: i64,
	pub data: Value,
	pub seq: Option<i64>,
	pub raw: String,
	pub payload: Vec<u8>,
	pub at: Instant,
}

impl VoiceFrame {
	pub(crate) fn new(connection: u64, frame: &Value, raw: String) -> Self {
		Self {
			connection,
			op: frame["op"].as_i64().unwrap_or(-1),
			data: frame["d"].clone(),
			seq: frame["seq"].as_i64(),
			raw,
			payload: Vec::new(),
			at: Instant::now(),
		}
	}

	pub(crate) fn binary(connection: u64, op: u8, seq: Option<i64>, payload: &[u8]) -> Self {
		Self {
			connection,
			op: i64::from(op),
			data: Value::Null,
			seq,
			raw: String::new(),
			payload: payload.to_vec(),
			at: Instant::now(),
		}
	}

	pub fn is_binary(&self) -> bool {
		self.raw.is_empty()
	}

	pub fn keys(&self, path: &[&str]) -> Vec<String> {
		field_names(&self.raw, path)
	}
}

#[derive(Debug, Clone)]
pub struct VoicePacket {
	pub ssrc: u32,
	pub mode: String,
	pub payload: Vec<u8>,
	pub opened: Option<Vec<u8>>,
	pub at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct VoiceRecording {
	pub sockets: Vec<VoiceSocket>,
	pub frames: Vec<VoiceFrame>,
	pub sent: Vec<VoiceFrame>,
	pub closes: Vec<(u64, Option<u16>)>,
	pub packets: Vec<VoicePacket>,
}

impl VoiceRecording {
	pub fn frames_with(&self, op: i64) -> impl Iterator<Item = &VoiceFrame> {
		self.frames.iter().filter(move |frame| frame.op == op)
	}

	pub fn sent_with(&self, op: i64) -> impl Iterator<Item = &VoiceFrame> {
		self.sent.iter().filter(move |frame| frame.op == op)
	}

	pub fn packets_from(&self, ssrc: u32) -> impl Iterator<Item = &VoicePacket> {
		self.packets.iter().filter(move |packet| packet.ssrc == ssrc)
	}

	pub fn socket(&self, id: u64) -> Option<&VoiceSocket> {
		self.sockets.iter().find(|socket| socket.id == id)
	}

	#[cfg(feature = "tls")]
	pub(crate) fn socket_mut(&mut self, id: u64) -> Option<&mut VoiceSocket> {
		self.sockets.iter_mut().find(|socket| socket.id == id)
	}
}
