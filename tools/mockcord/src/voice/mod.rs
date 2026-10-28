mod echo;
mod listener;
mod recording;

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use axum::Router;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use axum::routing::get;
use parking_lot::Mutex;
use rand::Rng;
use serde_json::{Value, json};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

use crate::json::parse_id;

pub use recording::{VoiceFrame, VoicePacket, VoiceRecording, VoiceSocket};

use listener::{ConnectionId, VoiceListener};

pub const VOICE_TOKEN: &str = "mock-voice-token";
pub const AES_GCM_MODE: &str = "aead_aes256_gcm_rtpsize";
pub const XCHACHA_MODE: &str = "aead_xchacha20_poly1305_rtpsize";

const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 41_250;
const FIRST_SSRC: u32 = 4_000;
const SSRC_STRIDE: u32 = 2;
const SCRIPTED: u64 = 0;
const KEY_LEN: usize = 32;
const BROADCAST_CAPACITY: usize = 64;
const AUTHENTICATION_FAILED: u16 = 4004;
const SESSION_INVALID: u16 = 4006;
const UNKNOWN_ENCRYPTION_MODE: u16 = 4016;

#[derive(Clone, Copy, Debug)]
enum Command {
	Close(u16),
	Drop,
}

struct Settings {
	modes: Vec<String>,
	heartbeat_interval_ms: u64,
}

struct Session {
	user_id: Option<u64>,
	ssrc: u32,
	seq: i64,
}

struct Voice {
	key: [u8; KEY_LEN],
	udp_port: u16,
	root_pem: Option<String>,
	next_ssrc: AtomicU32,
	next_connection: AtomicU64,
	broadcasts: broadcast::Sender<(u64, Value)>,
	commands: broadcast::Sender<Command>,
	settings: Mutex<Settings>,
	sessions: Mutex<HashMap<String, Session>>,
	modes: Mutex<HashMap<u32, String>>,
	log: Mutex<VoiceRecording>,
	revision: watch::Sender<u64>,
	shutdown: watch::Receiver<bool>,
}

impl Voice {
	fn record(&self, write: impl FnOnce(&mut VoiceRecording)) {
		write(&mut self.log.lock());
		self.revision.send_modify(|revision| *revision += 1);
	}

	fn sequenced(&self, session_id: Option<&str>, mut frame: Value) -> Value {
		let seq = session_id.and_then(|session_id| {
			let mut sessions = self.sessions.lock();
			let session = sessions.get_mut(session_id)?;
			session.seq += 1;
			Some(session.seq)
		});
		if let (Some(seq), Some(object)) = (seq, frame.as_object_mut()) {
			object.insert("seq".to_owned(), json!(seq));
		}
		frame
	}
}

#[derive(Clone)]
pub struct VoiceControl(Arc<Voice>);

impl VoiceControl {
	pub fn root_pem(&self) -> Option<&str> {
		self.0.root_pem.as_deref()
	}

	pub fn recording(&self) -> VoiceRecording {
		self.0.log.lock().clone()
	}

	pub fn subscribe(&self) -> watch::Receiver<u64> {
		self.0.revision.subscribe()
	}

	pub fn set_modes(&self, modes: &[&str]) {
		self.0.settings.lock().modes = modes.iter().map(|mode| (*mode).to_owned()).collect();
	}

	pub fn set_heartbeat_interval(&self, interval_ms: u64) {
		self.0.settings.lock().heartbeat_interval_ms = interval_ms;
	}

	pub fn close_sockets(&self, code: u16) {
		let _ = self.0.commands.send(Command::Close(code));
	}

	pub fn drop_sockets(&self) {
		let _ = self.0.commands.send(Command::Drop);
	}

	pub fn speak_as(&self, user_id: u64, ssrc: u32, speaking: bool) {
		let frame = json!({
			"op": 5,
			"d": {
				"speaking": u32::from(speaking),
				"ssrc": ssrc,
				"user_id": user_id.to_string(),
			},
		});
		let _ = self.0.broadcasts.send((SCRIPTED, frame));
	}
}

pub struct VoiceEndpoint {
	pub url: String,
	pub control: VoiceControl,
	pub tasks: Vec<JoinHandle<()>>,
}

pub async fn spawn(shutdown: watch::Receiver<bool>) -> io::Result<VoiceEndpoint> {
	let socket = UdpSocket::bind(("127.0.0.1", 0)).await?;
	let bound = listener::bind().await?;
	let voice = Arc::new(Voice {
		key: rand::rng().random(),
		udp_port: socket.local_addr()?.port(),
		root_pem: bound.root_pem.clone(),
		next_ssrc: AtomicU32::new(FIRST_SSRC),
		next_connection: AtomicU64::new(1),
		broadcasts: broadcast::channel(BROADCAST_CAPACITY).0,
		commands: broadcast::channel(BROADCAST_CAPACITY).0,
		settings: Mutex::new(Settings {
			modes: vec![AES_GCM_MODE.to_owned(), XCHACHA_MODE.to_owned()],
			heartbeat_interval_ms: DEFAULT_HEARTBEAT_INTERVAL_MS,
		}),
		sessions: Mutex::new(HashMap::new()),
		modes: Mutex::new(HashMap::new()),
		log: Mutex::new(VoiceRecording::default()),
		revision: watch::Sender::new(0),
		shutdown: shutdown.clone(),
	});
	let url = bound.url.clone();
	let (listener, accepting) = VoiceListener::start(bound, voice.clone());
	let router = Router::new().route("/", get(upgrade)).with_state(voice.clone());
	let mut graceful = shutdown;
	let serve = axum::serve(
		listener,
		router.into_make_service_with_connect_info::<ConnectionId>(),
	)
	.with_graceful_shutdown(async move {
		let _ = graceful.wait_for(|stopped| *stopped).await;
	});
	let signaling = tokio::spawn(async move {
		if let Err(error) = serve.await {
			tracing::error!(%error, "mock voice server stopped");
		}
	});
	let media = tokio::spawn(echo::run(socket, voice.clone()));
	Ok(VoiceEndpoint {
		url,
		control: VoiceControl(voice),
		tasks: vec![accepting, signaling, media],
	})
}

async fn upgrade(
	State(voice): State<Arc<Voice>>,
	ConnectInfo(connection): ConnectInfo<ConnectionId>,
	socket: WebSocketUpgrade,
) -> Response {
	socket.on_upgrade(move |socket| signal(voice, connection.0, socket))
}

struct Peer {
	connection: u64,
	session_id: Option<String>,
}

enum Flow {
	Continue,
	Close(u16),
	Stop,
}

async fn send(voice: &Voice, socket: &mut WebSocket, peer: &Peer, frame: Value) -> bool {
	let text = frame.to_string();
	let sent = socket.send(Message::Text(text.clone().into())).await.is_ok();
	if sent {
		voice.record(|log| log.sent.push(VoiceFrame::new(peer.connection, &frame, text)));
	}
	sent
}

async fn close(socket: &mut WebSocket, code: u16) {
	let frame = CloseFrame {
		code,
		reason: "".into(),
	};
	let _ = socket.send(Message::Close(Some(frame))).await;
}

async fn signal(voice: Arc<Voice>, connection: u64, mut socket: WebSocket) {
	let mut peer = Peer {
		connection,
		session_id: None,
	};
	let interval = voice.settings.lock().heartbeat_interval_ms;
	let hello = json!({ "op": 8, "d": { "heartbeat_interval": interval } });
	if !send(&voice, &mut socket, &peer, hello).await {
		return;
	}
	let mut broadcasts = voice.broadcasts.subscribe();
	let mut commands = voice.commands.subscribe();
	let mut shutdown = voice.shutdown.clone();
	loop {
		let flow = tokio::select! {
			incoming = socket.recv() => match incoming {
				Some(Ok(Message::Text(text))) => on_text(&voice, &mut socket, &mut peer, text.as_str()).await,
				Some(Ok(Message::Close(frame))) => {
					let code = frame.map(|frame| frame.code);
					voice.record(|log| log.closes.push((connection, code)));
					Flow::Stop
				}
				Some(Err(_)) | None => Flow::Stop,
				Some(Ok(_)) => Flow::Continue,
			},
			broadcast = broadcasts.recv() => match broadcast {
				Ok((origin, frame)) if origin != peer.connection && peer.session_id.is_some() => {
					let frame = voice.sequenced(peer.session_id.as_deref(), frame);
					if send(&voice, &mut socket, &peer, frame).await { Flow::Continue } else { Flow::Stop }
				}
				Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => Flow::Continue,
				Err(broadcast::error::RecvError::Closed) => Flow::Stop,
			},
			command = commands.recv() => match command {
				Ok(Command::Close(code)) => Flow::Close(code),
				Ok(Command::Drop) => Flow::Stop,
				Err(broadcast::error::RecvError::Lagged(_)) => Flow::Continue,
				Err(broadcast::error::RecvError::Closed) => Flow::Stop,
			},
			_ = shutdown.changed() => Flow::Stop,
		};
		match flow {
			Flow::Continue => {}
			Flow::Close(code) => {
				close(&mut socket, code).await;
				return;
			}
			Flow::Stop => return,
		}
	}
}

async fn on_text(voice: &Voice, socket: &mut WebSocket, peer: &mut Peer, text: &str) -> Flow {
	let Ok(frame) = serde_json::from_str::<Value>(text) else {
		return Flow::Continue;
	};
	voice.record(|log| {
		log.frames
			.push(VoiceFrame::new(peer.connection, &frame, text.to_owned()))
	});
	let data = &frame["d"];
	let reply = match frame["op"].as_u64() {
		Some(0) => match identify(voice, peer, data) {
			Ok(ready) => ready,
			Err(code) => return Flow::Close(code),
		},
		Some(1) => match select_protocol(voice, peer, data) {
			Ok(description) => description,
			Err(code) => return Flow::Close(code),
		},
		Some(3) => json!({ "op": 6, "d": { "t": data["t"] } }),
		Some(5) => {
			speaking(voice, peer, data);
			return Flow::Continue;
		}
		Some(7) => match resume(voice, peer, data) {
			Ok(resumed) => resumed,
			Err(code) => return Flow::Close(code),
		},
		_ => return Flow::Continue,
	};
	if send(voice, socket, peer, reply).await {
		Flow::Continue
	} else {
		Flow::Stop
	}
}

fn identify(voice: &Voice, peer: &mut Peer, data: &Value) -> Result<Value, u16> {
	if data["token"].as_str() != Some(VOICE_TOKEN) {
		return Err(AUTHENTICATION_FAILED);
	}
	let session_id = data["session_id"].as_str().unwrap_or_default().to_owned();
	let ssrc = voice.next_ssrc.fetch_add(SSRC_STRIDE, Ordering::Relaxed);
	voice.sessions.lock().insert(
		session_id.clone(),
		Session {
			user_id: parse_id(&data["user_id"]),
			ssrc,
			seq: -1,
		},
	);
	peer.session_id = Some(session_id);
	let modes = voice.settings.lock().modes.clone();
	Ok(json!({
		"op": 2,
		"d": {
			"ssrc": ssrc,
			"ip": "127.0.0.1",
			"port": voice.udp_port,
			"modes": modes,
			"experiments": ["fixed_keyframe_interval"],
			"streams": [],
		},
	}))
}

fn select_protocol(voice: &Voice, peer: &Peer, data: &Value) -> Result<Value, u16> {
	let mode = data["data"]["mode"].as_str().unwrap_or_default();
	if !voice.settings.lock().modes.iter().any(|offered| offered == mode) {
		return Err(UNKNOWN_ENCRYPTION_MODE);
	}
	let ssrc = peer
		.session_id
		.as_ref()
		.and_then(|session_id| voice.sessions.lock().get(session_id).map(|session| session.ssrc))
		.ok_or(SESSION_INVALID)?;
	voice.modes.lock().insert(ssrc, mode.to_owned());
	Ok(json!({
		"op": 4,
		"d": {
			"audio_codec": "opus",
			"video_codec": "H264",
			"media_session_id": format!("mock-media-{ssrc}"),
			"mode": mode,
			"secret_key": voice.key.to_vec(),
			"dave_protocol_version": 0,
		},
	}))
}

fn speaking(voice: &Voice, peer: &Peer, data: &Value) {
	let Some(session_id) = peer.session_id.as_ref() else {
		return;
	};
	let Some((user_id, ssrc)) = voice
		.sessions
		.lock()
		.get(session_id)
		.map(|session| (session.user_id, session.ssrc))
	else {
		return;
	};
	let frame = json!({
		"op": 5,
		"d": {
			"speaking": data["speaking"],
			"ssrc": ssrc,
			"user_id": user_id.map(|id| id.to_string()),
		},
	});
	let _ = voice.broadcasts.send((peer.connection, frame));
}

fn resume(voice: &Voice, peer: &mut Peer, data: &Value) -> Result<Value, u16> {
	if data["token"].as_str() != Some(VOICE_TOKEN) {
		return Err(AUTHENTICATION_FAILED);
	}
	let session_id = data["session_id"].as_str().unwrap_or_default();
	if !voice.sessions.lock().contains_key(session_id) {
		return Err(SESSION_INVALID);
	}
	peer.session_id = Some(session_id.to_owned());
	Ok(json!({ "op": 9, "d": null }))
}
