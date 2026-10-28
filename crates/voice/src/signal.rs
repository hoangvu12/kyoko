use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dave::Wire;
use futures_util::StreamExt;
use net::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderName, HeaderValue, ORIGIN, USER_AGENT};
use net::ws::{self, Limits, Message, WebSocket};
use net::{Client, Destination, Trust};
use protocol::fingerprint::{WEB_ORIGIN, accept_language};
use protocol::{Snowflake, Token};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::time::{Interval, MissedTickBehavior, interval_at, timeout};
use tracing::debug;
use url::{Host, Url};

use crate::crypto::EncryptionMode;
use crate::error::{Result, VoiceError};
use crate::transport::{Discovered, Transport};
use crate::wire::{
	self, CAMERA_LAYERS, CODECS, DAVE_PROTOCOL_VERSION, HeartbeatAck, Hello, Identify, Outbound,
	ProtocolData, RawFrame, Ready, SelectProtocol, SessionDescription, SinkWants, SpeakingUpdate, opcode,
};

const MAX_FRAME_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_HEARTBEAT_PERIOD: Duration = Duration::from_millis(5_000);
const MIN_HEARTBEAT_PERIOD: Duration = Duration::from_millis(1);
const MAX_BACKLOG_FRAMES: usize = 64;
const NORMAL_CLOSE: u16 = 1000;
const FULL_QUALITY: u8 = 100;
const VOICE_FLAG: u32 = 1;

pub(crate) struct Credentials {
	server_id: Snowflake,
	channel_id: Snowflake,
	user_id: Snowflake,
	session_id: String,
	token: Token,
	url: Url,
	client: Client,
	headers: HeaderMap,
}

pub(crate) struct Browser<'a> {
	pub user_agent: &'a str,
	pub locale: &'a str,
	pub trust: &'a Trust,
}

impl Credentials {
	pub fn user_id(&self) -> Snowflake {
		self.user_id
	}

	pub fn new(
		ids: [Snowflake; 3],
		session_id: String,
		token: Token,
		endpoint: &str,
		browser: &Browser<'_>,
	) -> Result<Self> {
		let [server_id, channel_id, user_id] = ids;
		let client = net::client_builder(Destination::Fetch, browser.trust)
			.and_then(|builder| builder.build())
			.map_err(|error| VoiceError::Transport(error.to_string()))?;
		Ok(Self {
			server_id,
			channel_id,
			user_id,
			session_id,
			token,
			url: endpoint_url(endpoint)?,
			client,
			headers: upgrade_headers(browser),
		})
	}

	fn identify(&self) -> Outbound<Identify<'_>> {
		Outbound {
			op: opcode::IDENTIFY,
			d: Identify {
				server_id: self.server_id,
				channel_id: self.channel_id,
				user_id: self.user_id,
				session_id: &self.session_id,
				token: self.token.as_str(),
				video: true,
				streams: CAMERA_LAYERS,
				max_dave_protocol_version: DAVE_PROTOCOL_VERSION,
			},
		}
	}

	fn resume(&self, seq_ack: i64) -> Outbound<wire::Resume<'_>> {
		Outbound {
			op: opcode::RESUME,
			d: wire::Resume {
				server_id: self.server_id,
				channel_id: self.channel_id,
				session_id: &self.session_id,
				token: self.token.as_str(),
				seq_ack,
			},
		}
	}
}

fn upgrade_headers(browser: &Browser<'_>) -> HeaderMap {
	let mut headers = HeaderMap::new();
	insert(&mut headers, USER_AGENT, browser.user_agent);
	headers.insert(ORIGIN, HeaderValue::from_static(WEB_ORIGIN));
	insert(&mut headers, ACCEPT_LANGUAGE, &accept_language(browser.locale));
	headers
}

fn insert(headers: &mut HeaderMap, name: HeaderName, value: &str) {
	if let Ok(value) = HeaderValue::from_str(value) {
		headers.insert(name, value);
	}
}

fn endpoint_url(endpoint: &str) -> Result<Url> {
	let raw = if endpoint.contains("://") {
		endpoint.to_owned()
	} else {
		format!("wss://{endpoint}")
	};
	let mut url = Url::parse(&raw).map_err(|_| VoiceError::InvalidEndpoint)?;
	let secure = match url.scheme() {
		"wss" => true,
		"ws" => is_loopback(&url),
		_ => false,
	};
	if !secure {
		return Err(VoiceError::InsecureEndpoint);
	}
	url.set_path("/");
	url.query_pairs_mut()
		.clear()
		.append_pair("v", wire::GATEWAY_VERSION);
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

#[derive(Debug)]
pub(crate) enum Exit {
	Closed(Option<u16>),
	Lost,
}

fn exit_error(exit: Exit) -> VoiceError {
	match exit {
		Exit::Closed(code) => VoiceError::Closed(code.unwrap_or(0)),
		Exit::Lost => VoiceError::Transport("connection lost".to_owned()),
	}
}

pub(crate) enum Body {
	Json(Value),
	Binary(Vec<u8>),
}

pub(crate) struct Frame {
	pub op: u8,
	pub body: Body,
}

struct Heartbeat {
	timer: Interval,
	nonce: u64,
	sent_at: Instant,
	awaiting: bool,
}

pub(crate) struct Link {
	socket: WebSocket,
	heartbeat: Option<Heartbeat>,
	pub last_seq: i64,
	pub ping_ms: Option<u32>,
	pub backlog: VecDeque<Frame>,
}

impl Link {
	async fn connect(credentials: &Credentials) -> Result<Self> {
		let limits = Limits {
			max_frame: MAX_FRAME_BYTES,
			max_message: MAX_FRAME_BYTES,
		};
		let socket = ws::connect(
			&credentials.client,
			credentials.url.as_str(),
			credentials.headers.clone(),
			limits,
			CONNECT_TIMEOUT,
		)
		.await
		.map_err(|error| VoiceError::Transport(error.to_string()))?;
		Ok(Self {
			socket,
			heartbeat: None,
			last_seq: -1,
			ping_ms: None,
			backlog: VecDeque::new(),
		})
	}

	pub async fn send<T: Serialize>(&mut self, op: u8, data: T) -> std::result::Result<(), Exit> {
		let text = serde_json::to_string(&Outbound { op, d: data }).map_err(|_| Exit::Lost)?;
		self.send_text(text).await
	}

	async fn send_frame<T: Serialize>(&mut self, frame: &Outbound<T>) -> std::result::Result<(), Exit> {
		let text = serde_json::to_string(frame).map_err(|_| Exit::Lost)?;
		self.send_text(text).await
	}

	async fn send_text(&mut self, text: String) -> std::result::Result<(), Exit> {
		self.socket
			.send(Message::text(text))
			.await
			.map_err(|_| Exit::Lost)
	}

	pub async fn send_wire(&mut self, wire: Wire) -> std::result::Result<(), Exit> {
		match wire {
			Wire::Text(text) => self.send_text(text).await,
			Wire::Binary(bytes) => self
				.socket
				.send(Message::binary(bytes))
				.await
				.map_err(|_| Exit::Lost),
		}
	}

	pub async fn close(&mut self) {
		let _ = timeout(CLOSE_TIMEOUT, ws::close(&mut self.socket, NORMAL_CLOSE, "")).await;
	}

	pub async fn next(&mut self) -> std::result::Result<Frame, Exit> {
		match self.backlog.pop_front() {
			Some(frame) => Ok(frame),
			None => self.read().await,
		}
	}

	async fn read(&mut self) -> std::result::Result<Frame, Exit> {
		loop {
			tokio::select! {
				message = self.socket.next() => match message {
					Some(Ok(message)) => {
						if let Some(frame) = self.on_message(message).await? {
							return Ok(frame);
						}
					}
					Some(Err(error)) => {
						debug!(error = %error.without_uri(), "voice websocket failed");
						return Err(Exit::Lost);
					}
					None => return Err(Exit::Lost),
				},
				() = next_beat(&mut self.heartbeat) => self.beat().await?,
			}
		}
	}

	async fn expect<T: DeserializeOwned>(&mut self, op: u8) -> Result<T> {
		loop {
			let frame = self.read().await.map_err(exit_error)?;
			if frame.op != op {
				self.defer(frame);
				continue;
			}
			let Body::Json(data) = frame.body else {
				continue;
			};
			return serde_json::from_value(data)
				.map_err(|_| VoiceError::Handshake("malformed voice gateway payload"));
		}
	}

	fn defer(&mut self, frame: Frame) {
		let replayable = matches!(
			frame.op,
			opcode::SPEAKING | opcode::CLIENTS_CONNECT | opcode::CLIENT_DISCONNECT | opcode::DAVE_FIRST
				..=opcode::DAVE_LAST
		);
		if replayable && self.backlog.len() < MAX_BACKLOG_FRAMES {
			self.backlog.push_back(frame);
		}
	}

	fn on_binary(&mut self, bytes: &[u8]) -> Option<Frame> {
		let (sequence, rest) = bytes.split_first_chunk::<2>()?;
		let (&op, payload) = rest.split_first()?;
		self.last_seq = i64::from(u16::from_be_bytes(*sequence));
		Some(Frame {
			op,
			body: Body::Binary(payload.to_vec()),
		})
	}

	async fn on_message(&mut self, message: Message) -> std::result::Result<Option<Frame>, Exit> {
		let text = match message {
			Message::Text(text) => text,
			Message::Binary(bytes) => return Ok(self.on_binary(&bytes)),
			Message::Close(frame) => return Err(Exit::Closed(frame.map(|frame| u16::from(frame.code)))),
			_ => return Ok(None),
		};
		let Ok(raw) = serde_json::from_str::<RawFrame>(text.as_str()) else {
			return Ok(None);
		};
		if let Some(seq) = raw.seq {
			self.last_seq = seq;
		}
		let Some(op) = raw.op else {
			return Ok(None);
		};
		match op {
			opcode::HELLO => {
				if let Ok(hello) = serde_json::from_value::<Hello>(raw.d) {
					self.start_heartbeat(hello.heartbeat_interval);
				}
				Ok(None)
			}
			opcode::HEARTBEAT => {
				self.beat().await?;
				Ok(None)
			}
			op => {
				if op == opcode::HEARTBEAT_ACK {
					self.on_ack(&raw.d);
				}
				Ok(Some(Frame {
					op,
					body: Body::Json(raw.d),
				}))
			}
		}
	}

	fn start_heartbeat(&mut self, interval_ms: f64) {
		let period = heartbeat_period(interval_ms);
		let mut timer = interval_at(tokio::time::Instant::now() + period, period);
		timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
		self.heartbeat = Some(Heartbeat {
			timer,
			nonce: 0,
			sent_at: Instant::now(),
			awaiting: false,
		});
	}

	fn on_ack(&mut self, data: &Value) {
		let Some(heartbeat) = self.heartbeat.as_mut() else {
			return;
		};
		let Ok(ack) = serde_json::from_value::<HeartbeatAck>(data.clone()) else {
			return;
		};
		if ack.t == Some(heartbeat.nonce) {
			heartbeat.awaiting = false;
			self.ping_ms = Some(heartbeat.sent_at.elapsed().as_millis() as u32);
		}
	}

	async fn beat(&mut self) -> std::result::Result<(), Exit> {
		let seq_ack = self.last_seq;
		let Some(heartbeat) = self.heartbeat.as_mut() else {
			return Ok(());
		};
		if heartbeat.awaiting {
			return Err(Exit::Lost);
		}
		heartbeat.nonce = unix_millis();
		heartbeat.sent_at = Instant::now();
		heartbeat.awaiting = true;
		let nonce = heartbeat.nonce;
		self.send(opcode::HEARTBEAT, wire::Heartbeat { t: nonce, seq_ack })
			.await
	}
}

fn heartbeat_period(interval_ms: f64) -> Duration {
	Duration::from_secs_f64(interval_ms.max(0.0) / 1000.0).clamp(MIN_HEARTBEAT_PERIOD, MAX_HEARTBEAT_PERIOD)
}

async fn next_beat(heartbeat: &mut Option<Heartbeat>) {
	match heartbeat {
		Some(heartbeat) => {
			heartbeat.timer.tick().await;
		}
		None => std::future::pending().await,
	}
}

fn unix_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |elapsed| elapsed.as_millis() as u64)
}

pub(crate) struct Established {
	pub link: Link,
	pub transport: Arc<dyn Transport>,
	pub mode: EncryptionMode,
	pub dave_version: u8,
}

pub(crate) async fn establish(credentials: &Credentials) -> Result<Established> {
	timeout(HANDSHAKE_TIMEOUT, handshake(credentials))
		.await
		.map_err(|_| VoiceError::Timeout)?
}

async fn handshake(credentials: &Credentials) -> Result<Established> {
	let mut link = Link::connect(credentials).await?;
	link.send_frame(&credentials.identify())
		.await
		.map_err(exit_error)?;
	let ready: Ready = link.expect(opcode::READY).await?;
	let mode = EncryptionMode::select(&ready.modes).ok_or(VoiceError::NoEncryptionMode)?;
	let discovered = Discovered::open(&ready.ip, ready.port, ready.ssrc).await?;
	let rtc_connection_id = uuid::Uuid::new_v4().to_string();
	let select = SelectProtocol {
		protocol: "udp",
		data: ProtocolData {
			address: &discovered.address,
			port: discovered.port,
			mode: mode.name(),
		},
		codecs: CODECS,
		rtc_connection_id: &rtc_connection_id,
		experiments: &ready.experiments,
	};
	link.send(opcode::SELECT_PROTOCOL, select)
		.await
		.map_err(exit_error)?;
	let description: SessionDescription = link.expect(opcode::SESSION_DESCRIPTION).await?;
	let mode = EncryptionMode::from_name(&description.mode).ok_or(VoiceError::NoEncryptionMode)?;
	let transport = discovered.secure(mode, &description.secret_key)?;
	link.send(opcode::SPEAKING, speaking(false, ready.ssrc))
		.await
		.map_err(exit_error)?;
	link.send(opcode::MEDIA_SINK_WANTS, SinkWants { any: FULL_QUALITY })
		.await
		.map_err(exit_error)?;
	Ok(Established {
		link,
		transport: Arc::new(transport),
		mode,
		dave_version: description.dave_protocol_version.unwrap_or_default(),
	})
}

pub(crate) async fn resume(credentials: &Credentials, last_seq: i64) -> Result<Link> {
	let attempt = async {
		let mut link = Link::connect(credentials).await?;
		link.last_seq = last_seq;
		link.send_frame(&credentials.resume(last_seq))
			.await
			.map_err(exit_error)?;
		loop {
			let frame = link.read().await.map_err(exit_error)?;
			if frame.op == opcode::RESUMED {
				return Ok(link);
			}
			link.defer(frame);
		}
	};
	timeout(HANDSHAKE_TIMEOUT, attempt)
		.await
		.map_err(|_| VoiceError::Timeout)?
}

pub(crate) fn speaking(speaking: bool, ssrc: u32) -> SpeakingUpdate {
	SpeakingUpdate {
		speaking: if speaking { VOICE_FLAG } else { 0 },
		delay: 0,
		ssrc,
	}
}
