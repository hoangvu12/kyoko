use serde::{Deserialize, Serialize};

use super::names;

pub const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const FRAME_HEADER: usize = 9;
const DATA: u8 = 0;
const HEADERS: u8 = 1;
const PRIORITY: u8 = 2;
const SETTINGS: u8 = 4;
const WINDOW_UPDATE: u8 = 8;
const CONTINUATION: u8 = 9;
const END_STREAM: u8 = 0x1;
const ACK: u8 = 0x1;
const END_HEADERS: u8 = 0x4;
const PADDED: u8 = 0x8;
const PRIORITY_FLAG: u8 = 0x20;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Http2Summary {
	pub akamai: String,
	pub preface: bool,
	pub frames: Vec<String>,
	pub settings: Vec<Setting>,
	pub window_update: Option<u32>,
	pub priority_frames: Vec<PriorityFrame>,
	pub requests: Vec<Http2Request>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
	pub id: u16,
	pub name: String,
	pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorityFrame {
	pub stream: u32,
	pub priority: StreamPriority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamPriority {
	pub exclusive: bool,
	pub depends_on: u32,
	pub weight: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Http2Request {
	pub stream: u32,
	pub flags: Vec<String>,
	pub priority: Option<StreamPriority>,
	pub pseudo_order: Vec<String>,
	pub headers: Vec<(String, String)>,
	pub cookie_fields: usize,
}

impl Http2Request {
	pub fn path(&self) -> Option<&str> {
		self.headers
			.iter()
			.find(|(name, _)| name == ":path")
			.map(|(_, value)| value.as_str())
	}
}

struct Frame {
	kind: u8,
	flags: u8,
	stream: u32,
	payload: Vec<u8>,
}

struct Pending {
	stream: u32,
	flags: u8,
	priority: Option<StreamPriority>,
	block: Vec<u8>,
}

pub struct Recorder {
	buffer: Vec<u8>,
	summary: Http2Summary,
	seen_headers: bool,
	pending: Option<Pending>,
	decoder: loona_hpack::Decoder<'static>,
	settings_to_ack: usize,
}

impl Default for Recorder {
	fn default() -> Recorder {
		Recorder {
			buffer: Vec::new(),
			summary: Http2Summary::default(),
			seen_headers: false,
			pending: None,
			decoder: loona_hpack::Decoder::new(),
			settings_to_ack: 0,
		}
	}
}

impl Recorder {
	pub fn feed(&mut self, bytes: &[u8]) -> Vec<Http2Request> {
		self.buffer.extend_from_slice(bytes);
		if !self.summary.preface {
			if self.buffer.len() < PREFACE.len() {
				return Vec::new();
			}
			if !self.buffer.starts_with(PREFACE) {
				self.buffer.clear();
				return Vec::new();
			}
			self.summary.preface = true;
			self.buffer.drain(..PREFACE.len());
		}
		let mut frames = Vec::new();
		let mut at = 0;
		while let Some((frame, length)) = frame_at(&self.buffer[at..]) {
			frames.push(frame);
			at += length;
		}
		self.buffer.drain(..at);
		let finished = frames.iter().filter_map(|frame| self.frame(frame)).collect();
		self.summary.akamai = akamai(&self.summary);
		finished
	}

	pub fn summary(&self) -> &Http2Summary {
		&self.summary
	}

	pub fn take_settings_acks(&mut self) -> usize {
		std::mem::take(&mut self.settings_to_ack)
	}

	fn frame(&mut self, frame: &Frame) -> Option<Http2Request> {
		if !self.seen_headers {
			let name = match frame.kind {
				SETTINGS if frame.flags & ACK != 0 => "SETTINGS_ACK".to_owned(),
				kind => frame_name(kind),
			};
			self.summary.frames.push(name);
		}
		match frame.kind {
			SETTINGS if frame.flags & ACK == 0 => {
				self.settings_to_ack += 1;
				if !self.seen_headers {
					self.summary.settings.extend(settings(&frame.payload));
				}
				None
			}
			WINDOW_UPDATE if frame.stream == 0 && !self.seen_headers => {
				if let Some(increment) = frame.payload.get(..4) {
					let value = u32::from_be_bytes([increment[0], increment[1], increment[2], increment[3]]);
					self.summary.window_update = Some(value & 0x7fff_ffff);
				}
				None
			}
			PRIORITY => {
				if let Some(priority) = priority(&frame.payload) {
					self.summary.priority_frames.push(PriorityFrame {
						stream: frame.stream,
						priority,
					});
				}
				None
			}
			HEADERS => {
				self.seen_headers = true;
				let (priority, block) = headers_block(frame.flags, &frame.payload)?;
				self.pending = Some(Pending {
					stream: frame.stream,
					flags: frame.flags,
					priority,
					block: block.to_vec(),
				});
				self.finish_if(frame.flags)
			}
			CONTINUATION => {
				let pending = self.pending.as_mut()?;
				pending.block.extend_from_slice(&frame.payload);
				self.finish_if(frame.flags)
			}
			_ => None,
		}
	}

	fn finish_if(&mut self, flags: u8) -> Option<Http2Request> {
		if flags & END_HEADERS == 0 {
			return None;
		}
		let pending = self.pending.take()?;
		let decoded = self.decoder.decode(&pending.block).ok()?;
		let headers = decoded
			.into_iter()
			.map(|(name, value)| {
				(
					String::from_utf8_lossy(&name).into_owned(),
					String::from_utf8_lossy(&value).into_owned(),
				)
			})
			.collect::<Vec<_>>();
		let request = Http2Request {
			stream: pending.stream,
			flags: header_flags(pending.flags),
			priority: pending.priority,
			pseudo_order: headers
				.iter()
				.filter(|(name, _)| name.starts_with(':'))
				.map(|(name, _)| name.clone())
				.collect(),
			cookie_fields: headers.iter().filter(|(name, _)| name == "cookie").count(),
			headers,
		};
		self.summary.requests.push(request.clone());
		Some(request)
	}
}

fn frame_at(bytes: &[u8]) -> Option<(Frame, usize)> {
	let header = bytes.get(..FRAME_HEADER)?;
	let length = u32::from_be_bytes([0, header[0], header[1], header[2]]) as usize;
	let payload = bytes.get(FRAME_HEADER..FRAME_HEADER + length)?;
	let stream = u32::from_be_bytes([header[5], header[6], header[7], header[8]]) & 0x7fff_ffff;
	Some((
		Frame {
			kind: header[3],
			flags: header[4],
			stream,
			payload: payload.to_vec(),
		},
		FRAME_HEADER + length,
	))
}

fn settings(payload: &[u8]) -> Vec<Setting> {
	payload
		.as_chunks::<6>()
		.0
		.iter()
		.map(|&[id_high, id_low, a, b, c, d]| {
			let id = u16::from_be_bytes([id_high, id_low]);
			Setting {
				id,
				name: names::setting(id),
				value: u32::from_be_bytes([a, b, c, d]),
			}
		})
		.collect()
}

fn priority(bytes: &[u8]) -> Option<StreamPriority> {
	let bytes = bytes.get(..5)?;
	let dependency = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
	Some(StreamPriority {
		exclusive: dependency & 0x8000_0000 != 0,
		depends_on: dependency & 0x7fff_ffff,
		weight: u16::from(bytes[4]) + 1,
	})
}

fn headers_block(flags: u8, payload: &[u8]) -> Option<(Option<StreamPriority>, &[u8])> {
	let mut body = payload;
	let mut padding = 0;
	if flags & PADDED != 0 {
		padding = usize::from(*body.first()?);
		body = body.get(1..)?;
	}
	let mut stream_priority = None;
	if flags & PRIORITY_FLAG != 0 {
		stream_priority = Some(priority(body)?);
		body = body.get(5..)?;
	}
	let end = body.len().checked_sub(padding)?;
	Some((stream_priority, &body[..end]))
}

fn header_flags(flags: u8) -> Vec<String> {
	[
		(END_STREAM, "END_STREAM"),
		(END_HEADERS, "END_HEADERS"),
		(PADDED, "PADDED"),
		(PRIORITY_FLAG, "PRIORITY"),
	]
	.into_iter()
	.filter(|(bit, _)| flags & bit != 0)
	.map(|(_, name)| name.to_owned())
	.collect()
}

fn frame_name(kind: u8) -> String {
	match kind {
		DATA => "DATA".to_owned(),
		HEADERS => "HEADERS".to_owned(),
		PRIORITY => "PRIORITY".to_owned(),
		3 => "RST_STREAM".to_owned(),
		SETTINGS => "SETTINGS".to_owned(),
		6 => "PING".to_owned(),
		7 => "GOAWAY".to_owned(),
		WINDOW_UPDATE => "WINDOW_UPDATE".to_owned(),
		CONTINUATION => "CONTINUATION".to_owned(),
		other => format!("0x{other:02x}"),
	}
}

fn akamai(summary: &Http2Summary) -> String {
	let settings = summary
		.settings
		.iter()
		.map(|setting| format!("{}:{}", setting.id, setting.value))
		.collect::<Vec<_>>()
		.join(";");
	let window = summary
		.window_update
		.map_or_else(|| "00".to_owned(), |value| value.to_string());
	let priorities = if summary.priority_frames.is_empty() {
		"0".to_owned()
	} else {
		summary
			.priority_frames
			.iter()
			.map(|frame| {
				format!(
					"{}:{}:{}:{}",
					frame.stream,
					u8::from(frame.priority.exclusive),
					frame.priority.depends_on,
					frame.priority.weight
				)
			})
			.collect::<Vec<_>>()
			.join(",")
	};
	let pseudo = summary
		.requests
		.first()
		.map(|request| {
			request
				.pseudo_order
				.iter()
				.filter_map(|name| name.chars().nth(1))
				.map(String::from)
				.collect::<Vec<_>>()
				.join(",")
		})
		.unwrap_or_default();
	format!("{settings}|{window}|{priorities}|{pseudo}")
}

pub fn server_settings() -> Vec<u8> {
	frame(SETTINGS, 0, 0, &[])
}

pub fn settings_ack() -> Vec<u8> {
	frame(SETTINGS, ACK, 0, &[])
}

pub fn response(stream: u32, status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
	let mut block = vec![match status {
		200 => 0x88,
		204 => 0x89,
		404 => 0x8d,
		_ => 0x8e,
	}];
	for (name, value) in headers {
		block.push(0x00);
		literal(&mut block, name.as_bytes());
		literal(&mut block, value.as_bytes());
	}
	let mut bytes = frame(HEADERS, END_HEADERS, stream, &block);
	bytes.extend(frame(DATA, END_STREAM, stream, body));
	bytes
}

fn literal(block: &mut Vec<u8>, bytes: &[u8]) {
	let mut length = bytes.len();
	if length < 127 {
		block.push(length as u8);
	} else {
		block.push(127);
		length -= 127;
		while length >= 128 {
			block.push((length % 128) as u8 | 0x80);
			length /= 128;
		}
		block.push(length as u8);
	}
	block.extend_from_slice(bytes);
}

fn frame(kind: u8, flags: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
	let length = (payload.len() as u32).to_be_bytes();
	let mut bytes = vec![length[1], length[2], length[3], kind, flags];
	bytes.extend_from_slice(&stream.to_be_bytes());
	bytes.extend_from_slice(payload);
	bytes
}
