use std::collections::BTreeMap;
use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::{FutureExt, SinkExt, StreamExt};
use protocol::Token;
use protocol::gateway::{DispatchEvent, GatewayPayload, Hello, Opcode, refreshed_token};
use serde_json::value::RawValue;
use tokio::sync::{Notify, mpsc, watch};
use tokio::time::{Instant, Interval, MissedTickBehavior, interval_at, sleep, sleep_until};
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tracing::{debug, warn};

use crate::config::GatewayConfig;
use crate::error::{FATAL_CLOSE_CODES, GatewayError, MESSAGE_TOO_BIG};
use crate::event::GatewayEvent;
use crate::frames;
use crate::outbox::Outbox;
use crate::transport::{self, Socket, ZlibStream};

const MIN_HEARTBEAT: Duration = Duration::from_millis(100);
const MAX_HEARTBEAT: Duration = Duration::from_secs(300);
const LIBRARY_CLOSE: u16 = 4000;
const NORMAL_CLOSE: u16 = 1000;
const REIDENTIFY_CLOSE_CODES: [u16; 3] = [4003, 4007, 4009];

struct Shutdown;

enum Ended {
	Reconnect {
		code: Option<u16>,
		reason: String,
		failure: bool,
	},
	Fatal {
		code: Option<u16>,
		reason: String,
		error: GatewayError,
	},
}

impl Ended {
	fn lost(reason: impl Into<String>) -> Self {
		Self::Reconnect {
			code: None,
			reason: reason.into(),
			failure: true,
		}
	}

	fn requested(code: u16, reason: &str, failure: bool) -> Self {
		Self::Reconnect {
			code: Some(code),
			reason: reason.to_owned(),
			failure,
		}
	}

	fn fatal(code: Option<u16>, reason: impl Into<String>, error: GatewayError) -> Self {
		Self::Fatal {
			code,
			reason: reason.into(),
			error,
		}
	}
}

struct Connection {
	socket: Socket,
	zlib: ZlibStream,
	heartbeat: Option<Interval>,
	awaiting_ack: bool,
	heartbeat_sent: Instant,
	ready: bool,
	identify_at: Option<Instant>,
}

impl Connection {
	fn new(socket: Socket) -> Self {
		Self {
			socket,
			zlib: ZlibStream::new(),
			heartbeat: None,
			awaiting_ack: false,
			heartbeat_sent: Instant::now(),
			ready: false,
			identify_at: None,
		}
	}
}

pub(crate) struct Session {
	config: GatewayConfig,
	events: mpsc::Sender<GatewayEvent>,
	outbox: Outbox,
	reconnect: Arc<Notify>,
	shutdown: watch::Receiver<bool>,
	latency: Arc<AtomicU64>,
	session_id: Option<String>,
	resume_url: Option<String>,
	seq: Option<u64>,
	failures: u32,
}

impl Session {
	pub fn new(
		config: GatewayConfig,
		events: mpsc::Sender<GatewayEvent>,
		urgent: mpsc::Receiver<String>,
		bulk: mpsc::Receiver<String>,
		reconnect: Arc<Notify>,
		shutdown: watch::Receiver<bool>,
		latency: Arc<AtomicU64>,
	) -> Self {
		let outbox = Outbox::new(config.tuning.send_budget, urgent, bulk);
		Self {
			config,
			events,
			outbox,
			reconnect,
			shutdown,
			latency,
			session_id: None,
			resume_url: None,
			seq: None,
			failures: 0,
		}
	}

	pub async fn run(mut self) {
		let _ = self.serve().await;
	}

	async fn serve(&mut self) -> Result<(), Shutdown> {
		loop {
			self.emit(GatewayEvent::Connecting {
				attempt: self.failures + 1,
				resuming: self.resumable().is_some(),
			})
			.await?;
			match self.connect_and_drive().await? {
				Ended::Reconnect {
					code,
					reason,
					failure,
				} => {
					if failure {
						self.failures += 1;
					}
					let retry_in = self.config.tuning.backoff.delay(self.failures);
					let will_resume = self.resumable().is_some();
					debug!(?code, will_resume, ?retry_in, "gateway disconnected");
					self.emit(GatewayEvent::Disconnected {
						code,
						reason,
						will_resume,
						attempt: self.failures + 1,
						retry_in,
					})
					.await?;
					self.pause(retry_in).await?;
				}
				Ended::Fatal { code, reason, error } => {
					warn!(?code, reason, %error, "gateway stopped");
					return self.emit(GatewayEvent::Fatal(error)).await;
				}
			}
		}
	}

	async fn connect_and_drive(&mut self) -> Result<Ended, Shutdown> {
		let url = match self.endpoint() {
			Ok(url) => url,
			Err(error) => return Ok(Ended::fatal(None, error.to_string(), error)),
		};
		let opened = tokio::select! {
			opened = transport::connect(&url, &self.config.identify) => opened,
			_ = stopped(&mut self.shutdown) => return Err(Shutdown),
		};
		let socket = match opened {
			Ok(socket) => socket,
			Err(error) => return Ok(Ended::lost(error.to_string())),
		};
		self.outbox.clear_control();
		self.reconnect.notified().now_or_never();
		self.drive(Connection::new(socket)).await
	}

	fn resumable(&self) -> Option<(&str, &str, u64)> {
		match (&self.session_id, &self.resume_url, self.seq) {
			(Some(session_id), Some(url), Some(seq)) => Some((session_id, url, seq)),
			_ => None,
		}
	}

	fn endpoint(&self) -> Result<url::Url, GatewayError> {
		let base = match self.resumable() {
			Some((_, url, _)) => url,
			None => &self.config.gateway_url,
		};
		transport::endpoint(base)
	}

	async fn drive(&mut self, mut connection: Connection) -> Result<Ended, Shutdown> {
		loop {
			let ended = tokio::select! {
				message = connection.socket.next() => self.on_socket(&mut connection, message).await?,
				_ = next_tick(&mut connection.heartbeat) => self.on_heartbeat_due(&mut connection).await,
				_ = until(connection.identify_at) => self.on_identify_due(&mut connection),
				frame = self.outbox.next(connection.ready) => match frame {
					Some(frame) => self.transmit(&mut connection, frame).await,
					None => return Err(Shutdown),
				},
				_ = self.reconnect.notified() => {
					debug!("gateway reconnect requested");
					Some(self.hangup(&mut connection, LIBRARY_CLOSE, "reconnect requested", false).await)
				}
				_ = stopped(&mut self.shutdown) => {
					transport::hangup(&mut connection.socket, NORMAL_CLOSE, "shutdown").await;
					return Err(Shutdown);
				}
			};
			if let Some(ended) = ended {
				return Ok(ended);
			}
		}
	}

	async fn on_socket(
		&mut self,
		connection: &mut Connection,
		message: Option<Result<Message, WebSocketError>>,
	) -> Result<Option<Ended>, Shutdown> {
		match message {
			None => Ok(Some(Ended::lost("connection closed"))),
			Some(Err(error)) => match transport::read_error(error) {
				GatewayError::OverSized => Ok(Some(self.oversized(connection).await)),
				error => Ok(Some(Ended::lost(error.to_string()))),
			},
			Some(Ok(message)) => self.on_message(connection, message).await,
		}
	}

	async fn on_message(
		&mut self,
		connection: &mut Connection,
		message: Message,
	) -> Result<Option<Ended>, Shutdown> {
		let payload = match message {
			Message::Binary(frame) => match connection.zlib.push(&frame) {
				Ok(Some(json)) => decode_payload(json),
				Ok(None) => return Ok(None),
				Err(GatewayError::OverSized) => return Ok(Some(self.oversized(connection).await)),
				Err(error) => {
					warn!(%error, "gateway stream corrupt");
					return Ok(Some(
						self.hangup(connection, LIBRARY_CLOSE, "corrupt stream", true)
							.await,
					));
				}
			},
			Message::Text(text) => decode_payload(text.as_bytes()),
			Message::Close(frame) => return Ok(Some(self.on_close(frame))),
			_ => return Ok(None),
		};
		match payload {
			Some(payload) => self.on_payload(connection, payload).await,
			None => Ok(None),
		}
	}

	fn on_close(&mut self, frame: Option<CloseFrame>) -> Ended {
		let code = frame.as_ref().map(|frame| u16::from(frame.code));
		let reason = frame.map(|frame| frame.reason.to_string()).unwrap_or_default();
		debug!(?code, "gateway close frame");
		match code {
			Some(code) if FATAL_CLOSE_CODES.contains(&code) => {
				return Ended::fatal(Some(code), reason, GatewayError::Fatal { code });
			}
			Some(code) if REIDENTIFY_CLOSE_CODES.contains(&code) => self.clear_session(),
			_ => {}
		}
		Ended::Reconnect {
			code,
			reason,
			failure: true,
		}
	}

	async fn on_payload(
		&mut self,
		connection: &mut Connection,
		payload: GatewayPayload,
	) -> Result<Option<Ended>, Shutdown> {
		let opcode = payload.opcode();
		debug!(?opcode, "gateway opcode");
		match opcode {
			Opcode::Dispatch => {
				self.on_dispatch(connection, payload).await?;
				Ok(None)
			}
			Opcode::Heartbeat => Ok(self.send_heartbeat(connection)),
			Opcode::Reconnect => Ok(Some(
				self.hangup(connection, LIBRARY_CLOSE, "reconnect requested", false)
					.await,
			)),
			Opcode::InvalidSession => {
				let resumable = payload
					.d
					.and_then(|data| serde_json::from_str::<bool>(data.get()).ok())
					.unwrap_or(false);
				if resumable && self.resumable().is_some() {
					return Ok(Some(
						self.hangup(connection, LIBRARY_CLOSE, "invalid session", false)
							.await,
					));
				}
				self.clear_session();
				connection.ready = false;
				connection.identify_at = Some(Instant::now() + identify_delay());
				self.emit(GatewayEvent::Connecting {
					attempt: self.failures + 1,
					resuming: false,
				})
				.await?;
				Ok(None)
			}
			Opcode::Hello => self.on_hello(connection, payload).await,
			Opcode::HeartbeatAck => {
				if !connection.awaiting_ack {
					return Ok(None);
				}
				connection.awaiting_ack = false;
				let rtt = connection.heartbeat_sent.elapsed();
				self.latency.store(
					u64::try_from(rtt.as_micros()).unwrap_or(u64::MAX - 1),
					Ordering::Relaxed,
				);
				self.emit(GatewayEvent::Latency { heartbeat_rtt: rtt }).await?;
				Ok(None)
			}
			_ => Ok(None),
		}
	}

	async fn on_hello(
		&mut self,
		connection: &mut Connection,
		payload: GatewayPayload,
	) -> Result<Option<Ended>, Shutdown> {
		let Some(hello) = payload
			.d
			.and_then(|data| serde_json::from_str::<Hello>(data.get()).ok())
		else {
			return Ok(Some(
				self.hangup(connection, LIBRARY_CLOSE, "malformed hello", true)
					.await,
			));
		};
		let interval = Duration::from_millis(hello.heartbeat_interval);
		self.emit(GatewayEvent::Hello {
			heartbeat_interval: interval,
		})
		.await?;
		let period = interval.clamp(MIN_HEARTBEAT, MAX_HEARTBEAT);
		let mut timer = interval_at(
			Instant::now() + period.mul_f64(rand::random_range(0.0..1.0)),
			period,
		);
		timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
		connection.heartbeat = Some(timer);
		Ok(self.authenticate())
	}

	fn authenticate(&mut self) -> Option<Ended> {
		let frame = match self.resumable() {
			Some((session_id, _, seq)) => {
				debug!("gateway resume");
				frames::resume(&self.config.token, session_id, seq)
			}
			None => {
				debug!("gateway identify");
				frames::identify(&self.config)
			}
		};
		self.queue_control(frame)
	}

	fn on_identify_due(&mut self, connection: &mut Connection) -> Option<Ended> {
		connection.identify_at = None;
		self.authenticate()
	}

	fn queue_control(&mut self, frame: Result<String, GatewayError>) -> Option<Ended> {
		match frame {
			Ok(frame) => {
				self.outbox.push_control(frame);
				None
			}
			Err(error) => Some(Ended::fatal(None, error.to_string(), error)),
		}
	}

	async fn on_heartbeat_due(&mut self, connection: &mut Connection) -> Option<Ended> {
		if connection.awaiting_ack {
			debug!("gateway heartbeat ack missing");
			return Some(
				self.hangup(connection, LIBRARY_CLOSE, "heartbeat ack missing", true)
					.await,
			);
		}
		self.send_heartbeat(connection)
	}

	fn send_heartbeat(&mut self, connection: &mut Connection) -> Option<Ended> {
		connection.awaiting_ack = true;
		connection.heartbeat_sent = Instant::now();
		self.queue_control(frames::heartbeat(self.seq))
	}

	async fn on_dispatch(
		&mut self,
		connection: &mut Connection,
		payload: GatewayPayload,
	) -> Result<(), Shutdown> {
		let Some(name) = payload.t.as_deref() else {
			return Ok(());
		};
		if let Some(seq) = payload.s {
			self.seq = Some(seq);
		}
		let data = payload.d.as_deref().unwrap_or(RawValue::NULL);
		if name == "READY" {
			log_ready_shape(data);
		}
		if name == "READY"
			&& let Some(token) = refreshed_token(data)
		{
			debug!("gateway token refreshed");
			self.config.token = Token::new(token.as_str());
			self.emit(GatewayEvent::TokenRefreshed(token)).await?;
		}
		let event = match DispatchEvent::parse(name, data) {
			Ok(event) => event,
			Err(error) => {
				warn!(event = name, category = ?error.category(), "gateway dispatch decode failed");
				return self
					.emit(GatewayEvent::DecodeFailure {
						event: name.to_owned(),
					})
					.await;
			}
		};
		debug!(event = name, "gateway dispatch");
		let event = match event {
			DispatchEvent::Ready(ready) => {
				self.session_id = Some(ready.session_id.clone()).filter(|id| !id.is_empty());
				self.resume_url = ready
					.resume_gateway_url
					.clone()
					.filter(|url| transport::endpoint(url).is_ok());
				connection.ready = true;
				self.failures = 0;
				GatewayEvent::Ready(ready)
			}
			DispatchEvent::ReadySupplemental(supplemental) => GatewayEvent::ReadySupplemental(supplemental),
			DispatchEvent::Resumed => {
				connection.ready = true;
				self.failures = 0;
				GatewayEvent::Resumed
			}
			event => GatewayEvent::Dispatch {
				seq: self.seq.unwrap_or(0),
				event,
			},
		};
		self.emit(event).await
	}

	async fn transmit(&mut self, connection: &mut Connection, frame: String) -> Option<Ended> {
		match connection.socket.send(Message::text(frame)).await {
			Ok(()) => None,
			Err(error) => Some(Ended::lost(transport::read_error(error).to_string())),
		}
	}

	async fn hangup(
		&mut self,
		connection: &mut Connection,
		code: u16,
		reason: &'static str,
		failure: bool,
	) -> Ended {
		transport::hangup(&mut connection.socket, code, reason).await;
		Ended::requested(code, reason, failure)
	}

	async fn oversized(&mut self, connection: &mut Connection) -> Ended {
		transport::hangup(&mut connection.socket, MESSAGE_TOO_BIG, "message too big").await;
		Ended::fatal(Some(MESSAGE_TOO_BIG), "message too big", GatewayError::OverSized)
	}

	fn clear_session(&mut self) {
		self.session_id = None;
		self.resume_url = None;
		self.seq = None;
	}

	async fn emit(&mut self, event: GatewayEvent) -> Result<(), Shutdown> {
		tokio::select! {
			sent = self.events.send(event) => sent.map_err(|_| Shutdown),
			_ = stopped(&mut self.shutdown) => Err(Shutdown),
		}
	}

	async fn pause(&mut self, duration: Duration) -> Result<(), Shutdown> {
		tokio::select! {
			_ = sleep(duration) => Ok(()),
			_ = stopped(&mut self.shutdown) => Err(Shutdown),
		}
	}
}

fn decode_payload(bytes: &[u8]) -> Option<GatewayPayload> {
	match serde_json::from_slice(bytes) {
		Ok(payload) => Some(payload),
		Err(error) => {
			debug!(category = ?error.classify(), "gateway frame undecodable");
			None
		}
	}
}

fn identify_delay() -> Duration {
	Duration::from_secs_f64(rand::random_range(1.0..=5.0))
}

async fn next_tick(timer: &mut Option<Interval>) {
	match timer {
		Some(timer) => {
			timer.tick().await;
		}
		None => pending().await,
	}
}

async fn until(deadline: Option<Instant>) {
	match deadline {
		Some(deadline) => sleep_until(deadline).await,
		None => pending().await,
	}
}

async fn stopped(shutdown: &mut watch::Receiver<bool>) {
	let _ = shutdown.wait_for(|stop| *stop).await;
}

fn log_ready_shape(data: &RawValue) {
	let Ok(fields) = serde_json::from_str::<BTreeMap<&str, &RawValue>>(data.get()) else {
		return;
	};
	let keys = fields.keys().copied().collect::<Vec<_>>().join(",");
	let version = fields.get("v").map(|value| value.get()).unwrap_or("none");
	debug!(%keys, version, auth_token = fields.contains_key("auth_token"), "gateway ready shape");
}
