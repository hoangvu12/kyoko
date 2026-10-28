use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use axum::http::HeaderMap;
use parking_lot::Mutex;
use protocol::Opcode;
use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;
use serde_json::value::RawValue;
use tokio::sync::watch;

use crate::scenario::path_matches;

pub type Headers = Vec<(String, String)>;

#[derive(Debug, Clone)]
pub struct RestRequest {
	pub method: String,
	pub version: u32,
	pub path: String,
	pub query: Option<String>,
	pub headers: Headers,
	pub body: Vec<u8>,
	pub status: u16,
	pub injected: bool,
	pub at: Instant,
}

impl RestRequest {
	pub fn header(&self, name: &str) -> Option<&str> {
		find_header(&self.headers, name)
	}

	pub fn json(&self) -> Option<Value> {
		serde_json::from_slice(&self.body).ok()
	}

	pub fn is(&self, method: &str, pattern: &str) -> bool {
		(method == "*" || self.method.eq_ignore_ascii_case(method)) && path_matches(pattern, &self.path)
	}
}

#[derive(Debug, Clone)]
pub struct MediaRequest {
	pub path: String,
	pub query: Option<String>,
	pub headers: Headers,
	pub status: u16,
	pub at: Instant,
}

impl MediaRequest {
	pub fn header(&self, name: &str) -> Option<&str> {
		find_header(&self.headers, name)
	}

	pub fn url(&self) -> String {
		match &self.query {
			Some(query) => format!("{}?{query}", self.path),
			None => self.path.clone(),
		}
	}

	pub fn param(&self, name: &str) -> Option<&str> {
		self.query.as_deref()?.split('&').find_map(|pair| {
			let (key, value) = pair.split_once('=')?;
			(key == name).then_some(value)
		})
	}
}

#[derive(Debug, Clone)]
pub struct GatewayConnection {
	pub id: u64,
	pub query: Option<String>,
	pub headers: Headers,
	pub at: Instant,
}

impl GatewayConnection {
	pub fn header(&self, name: &str) -> Option<&str> {
		find_header(&self.headers, name)
	}
}

#[derive(Debug, Clone)]
pub struct GatewayFrame {
	pub connection: u64,
	pub op: i64,
	pub data: Value,
	pub raw: String,
	pub at: Instant,
}

impl GatewayFrame {
	pub fn opcode(&self) -> Opcode {
		Opcode::from(self.op)
	}

	pub fn keys(&self, path: &[&str]) -> Vec<String> {
		field_names(&self.raw, path)
	}
}

pub(crate) fn field_names(raw: &str, path: &[&str]) -> Vec<String> {
	let mut raw = raw.to_owned();
	for segment in path {
		let Some(nested) = ordered_fields(&raw)
			.into_iter()
			.find_map(|(name, value)| (name == *segment).then_some(value))
		else {
			return Vec::new();
		};
		raw = nested;
	}
	ordered_fields(&raw).into_iter().map(|(name, _)| name).collect()
}

struct OrderedFields(Vec<(String, String)>);

impl<'de> Deserialize<'de> for OrderedFields {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		deserializer.deserialize_map(OrderedFieldsVisitor)
	}
}

struct OrderedFieldsVisitor;

impl<'de> Visitor<'de> for OrderedFieldsVisitor {
	type Value = OrderedFields;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("a json object")
	}

	fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedFields, A::Error> {
		let mut fields = Vec::new();
		while let Some((name, value)) = map.next_entry::<String, Box<RawValue>>()? {
			fields.push((name, value.get().to_owned()));
		}
		Ok(OrderedFields(fields))
	}
}

fn ordered_fields(raw: &str) -> Vec<(String, String)> {
	serde_json::from_str::<OrderedFields>(raw)
		.map(|fields| fields.0)
		.unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct GatewayClose {
	pub connection: u64,
	pub code: Option<u16>,
	pub at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct Recording {
	pub requests: Vec<RestRequest>,
	pub connections: Vec<GatewayConnection>,
	pub frames: Vec<GatewayFrame>,
	pub closes: Vec<GatewayClose>,
	pub media: Vec<MediaRequest>,
}

impl Recording {
	pub fn requests_to<'a>(
		&'a self,
		method: &'a str,
		pattern: &'a str,
	) -> impl Iterator<Item = &'a RestRequest> {
		self.requests
			.iter()
			.filter(move |request| request.is(method, pattern))
	}

	pub fn media_to<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a MediaRequest> {
		self.media
			.iter()
			.filter(move |request| request.path.starts_with(prefix))
	}

	pub fn frames_with(&self, opcode: Opcode) -> impl Iterator<Item = &GatewayFrame> {
		self.frames.iter().filter(move |frame| frame.opcode() == opcode)
	}
}

pub struct Recorder {
	enabled: AtomicBool,
	log: Mutex<Recording>,
	revision: watch::Sender<u64>,
}

impl Recorder {
	pub fn new(enabled: bool) -> Self {
		Self {
			enabled: AtomicBool::new(enabled),
			log: Mutex::new(Recording::default()),
			revision: watch::Sender::new(0),
		}
	}

	pub fn enabled(&self) -> bool {
		self.enabled.load(Ordering::Relaxed)
	}

	pub fn set_enabled(&self, enabled: bool) {
		self.enabled.store(enabled, Ordering::Relaxed);
	}

	pub fn clear(&self) {
		*self.log.lock() = Recording::default();
		self.bump();
	}

	pub fn snapshot(&self) -> Recording {
		self.log.lock().clone()
	}

	pub fn inspect<T>(&self, read: impl FnOnce(&Recording) -> T) -> T {
		read(&self.log.lock())
	}

	pub fn subscribe(&self) -> watch::Receiver<u64> {
		self.revision.subscribe()
	}

	pub fn request(&self, request: RestRequest) {
		self.append(|log| log.requests.push(request));
	}

	pub fn media(&self, request: MediaRequest) {
		self.append(|log| log.media.push(request));
	}

	pub fn connection(&self, connection: GatewayConnection) {
		self.append(|log| log.connections.push(connection));
	}

	pub fn frame(&self, frame: GatewayFrame) {
		self.append(|log| log.frames.push(frame));
	}

	pub fn close(&self, close: GatewayClose) {
		self.append(|log| log.closes.push(close));
	}

	fn append(&self, write: impl FnOnce(&mut Recording)) {
		if !self.enabled() {
			return;
		}
		write(&mut self.log.lock());
		self.bump();
	}

	fn bump(&self) {
		self.revision.send_modify(|revision| *revision += 1);
	}
}

pub fn header_pairs(headers: &HeaderMap) -> Headers {
	headers
		.iter()
		.map(|(name, value)| {
			(
				name.as_str().to_owned(),
				String::from_utf8_lossy(value.as_bytes()).into_owned(),
			)
		})
		.collect()
}

fn find_header<'a>(headers: &'a Headers, name: &str) -> Option<&'a str> {
	headers
		.iter()
		.find(|(key, _)| key.eq_ignore_ascii_case(name))
		.map(|(_, value)| value.as_str())
}
