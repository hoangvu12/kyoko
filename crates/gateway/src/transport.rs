use std::net::IpAddr;
use std::time::Duration;

use flate2::{Decompress, FlushDecompress};
use net::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue, ORIGIN, USER_AGENT};
use net::ws::{self, Limits};
use net::{Client, Destination, Trust};
use protocol::IdentifyProperties;
use protocol::fingerprint::{API_VERSION, WEB_ORIGIN, accept_language};
use tokio::time::timeout;
use url::{Host, Url};

use crate::error::GatewayError;

pub(crate) type Socket = ws::WebSocket;

const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MAX_DECOMPRESSED_BYTES: usize = 64 * 1024 * 1024;
const RETAINED_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const INFLATE_CHUNK_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const ZLIB_SYNC_SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

pub(crate) fn endpoint(base: &str) -> Result<Url, GatewayError> {
	let mut url = Url::parse(base).map_err(|_| GatewayError::InvalidUrl)?;
	let secure = match url.scheme() {
		"wss" => true,
		"ws" => is_loopback(&url),
		_ => false,
	};
	if !secure {
		return Err(GatewayError::InsecureUrl);
	}
	url.query_pairs_mut()
		.clear()
		.append_pair("v", &API_VERSION.to_string())
		.append_pair("encoding", "json")
		.append_pair("compress", "zlib-stream");
	Ok(url)
}

fn is_loopback(url: &Url) -> bool {
	match url.host() {
		Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
		Some(Host::Ipv4(address)) => IpAddr::V4(address).is_loopback(),
		Some(Host::Ipv6(address)) => IpAddr::V6(address).is_loopback(),
		None => false,
	}
}

pub(crate) fn client(trust: &Trust) -> Result<Client, GatewayError> {
	net::client_builder(Destination::Fetch, trust)
		.and_then(|builder| builder.build())
		.map_err(|error| GatewayError::Transport(error.to_string()))
}

pub(crate) async fn connect(
	client: &Client,
	url: &Url,
	identify: &IdentifyProperties,
) -> Result<Socket, GatewayError> {
	let mut headers = HeaderMap::new();
	insert(&mut headers, USER_AGENT, identify.user_agent());
	headers.insert(ORIGIN, HeaderValue::from_static(WEB_ORIGIN));
	insert(
		&mut headers,
		ACCEPT_LANGUAGE,
		&accept_language(&identify.system_locale),
	);
	let limits = Limits {
		max_frame: MAX_FRAME_BYTES,
		max_message: MAX_FRAME_BYTES,
	};
	ws::connect(client, url.as_str(), headers, limits, CONNECT_TIMEOUT)
		.await
		.map_err(|error| GatewayError::Transport(error.to_string()))
}

fn insert(headers: &mut HeaderMap, name: net::header::HeaderName, value: &str) {
	if let Ok(value) = HeaderValue::from_str(value) {
		headers.insert(name, value);
	}
}

pub(crate) fn read_error(error: net::Error) -> GatewayError {
	if ws::is_oversized(&error) {
		return GatewayError::OverSized;
	}
	GatewayError::Transport(error.without_uri().to_string())
}

pub(crate) async fn hangup(socket: &mut Socket, code: u16, reason: &'static str) {
	let _ = timeout(CLOSE_TIMEOUT, ws::close(socket, code, reason)).await;
}

pub(crate) struct ZlibStream {
	decompress: Decompress,
	pending: Vec<u8>,
	output: Vec<u8>,
}

impl ZlibStream {
	pub fn new() -> Self {
		Self {
			decompress: Decompress::new(true),
			pending: Vec::new(),
			output: Vec::new(),
		}
	}

	pub fn push(&mut self, frame: &[u8]) -> Result<Option<&[u8]>, GatewayError> {
		let complete = frame.ends_with(&ZLIB_SYNC_SUFFIX);
		if self.pending.is_empty() && complete {
			self.inflate(frame)?;
			return Ok(Some(&self.output));
		}
		if self.pending.len() + frame.len() > MAX_FRAME_BYTES {
			return Err(GatewayError::OverSized);
		}
		self.pending.extend_from_slice(frame);
		if !complete {
			return Ok(None);
		}
		let pending = std::mem::take(&mut self.pending);
		let result = self.inflate(&pending);
		self.pending = pending;
		self.pending.clear();
		result?;
		Ok(Some(&self.output))
	}

	fn inflate(&mut self, input: &[u8]) -> Result<(), GatewayError> {
		self.output.clear();
		self.output.shrink_to(RETAINED_OUTPUT_BYTES);
		let mut consumed = 0;
		loop {
			let length = self.output.len();
			let spare = length
				.max(INFLATE_CHUNK_BYTES)
				.min(MAX_DECOMPRESSED_BYTES + 1 - length);
			self.output.reserve_exact(spare);
			let input_before = self.decompress.total_in();
			let output_before = self.decompress.total_out();
			self.decompress
				.decompress_vec(&input[consumed..], &mut self.output, FlushDecompress::Sync)
				.map_err(|_| GatewayError::Decompress)?;
			let read = (self.decompress.total_in() - input_before) as usize;
			let produced = self.decompress.total_out() - output_before;
			consumed += read;
			if self.output.len() > MAX_DECOMPRESSED_BYTES {
				return Err(GatewayError::OverSized);
			}
			let drained = consumed == input.len() && self.output.len() < self.output.capacity();
			if drained || (read == 0 && produced == 0) {
				return Ok(());
			}
		}
	}
}
