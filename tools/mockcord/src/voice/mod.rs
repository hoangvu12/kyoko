mod assemble;
mod echo;
mod listener;
mod recording;
mod room;
mod stream;
mod video;

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use axum::Router;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use axum::routing::get;
use dave::Wire;
use dave::testing::Harness;
use parking_lot::Mutex;
use rand::Rng;
use serde_json::{Value, json};
use tokio::net::UdpSocket;
use tokio::sync::{Notify, broadcast, watch};
use tokio::task::JoinHandle;

use crate::json::parse_id;

pub use recording::{VoiceFrame, VoicePacket, VoiceRecording, VoiceRtcp, VoiceSocket, VoiceVideoFrame};
pub use stream::{MockStream, go_live};
pub use video::Cameras;
pub use video::{CAMERA_FPS, CAMERA_HEIGHT, CAMERA_WIDTH, ScriptedCamera, camera_frames, clip_frames};

use assemble::Assembler;
use listener::{ConnectionId, VoiceListener};
use room::{Mail, Rooms};

pub const VOICE_TOKEN: &str = "mock-voice-token";
pub const AES_GCM_MODE: &str = "aead_aes256_gcm_rtpsize";
pub const XCHACHA_MODE: &str = "aead_xchacha20_poly1305_rtpsize";

const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 41_250;
const FIRST_SSRC: u32 = 4_000;
const SSRC_STRIDE: u32 = 2;
const SSRCS_PER_STREAM: u32 = 2;
const MAX_OFFERED_STREAMS: usize = 4;
const RTCP_FEEDBACK_VERSION: u8 = 0x80;
const PICTURE_LOSS_FORMAT: u8 = 1;
const PAYLOAD_SPECIFIC_FEEDBACK: u8 = 206;
const PICTURE_LOSS_WORDS: u16 = 2;
const SCRIPTED: u64 = 0;
const KEY_LEN: usize = 32;
const BROADCAST_CAPACITY: usize = 64;
const MAIL_CAPACITY: usize = 512;
const OPUS_PAYLOAD_TYPE: u8 = 120;
const RTP_VERSION: u8 = 0x80;
const DAVE_TRANSITION_READY: u64 = 23;
const DAVE_INVALID_COMMIT_WELCOME: u64 = 31;
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
	dave_version: u8,
}

struct Session {
	user_id: Option<u64>,
	channel_id: Option<u64>,
	ssrc: u32,
	video_ssrcs: Vec<u32>,
	seq: i64,
}

impl Session {
	fn owns(&self, ssrc: u32) -> bool {
		self.ssrc == ssrc || self.video_ssrcs.contains(&ssrc)
	}
}

struct Voice {
	key: [u8; KEY_LEN],
	udp_port: u16,
	root_pem: Option<String>,
	next_ssrc: AtomicU32,
	next_connection: AtomicU64,
	broadcasts: broadcast::Sender<(u64, Option<u64>, Value)>,
	commands: broadcast::Sender<Command>,
	settings: Mutex<Settings>,
	sessions: Mutex<HashMap<String, Session>>,
	modes: Mutex<HashMap<u32, String>>,
	log: Mutex<VoiceRecording>,
	revision: watch::Sender<u64>,
	shutdown: watch::Receiver<bool>,
	echo_held: AtomicBool,
	held_echo: Mutex<Vec<(Vec<u8>, SocketAddr)>>,
	echo_released: Notify,
	udp: Arc<UdpSocket>,
	clients: Mutex<HashMap<u32, SocketAddr>>,
	media_counter: AtomicU32,
	rooms: Mutex<Rooms>,
	mail: broadcast::Sender<(u64, u64, Wire)>,
	cameras: Option<Cameras>,
	streams: Mutex<stream::Streams>,
	assembler: Mutex<Assembler>,
}

impl Voice {
	fn record(&self, write: impl FnOnce(&mut VoiceRecording)) {
		write(&mut self.log.lock());
		self.revision.send_modify(|revision| *revision += 1);
	}

	fn next_seq(&self, session_id: Option<&str>) -> Option<i64> {
		let mut sessions = self.sessions.lock();
		let session = sessions.get_mut(session_id?)?;
		session.seq += 1;
		Some(session.seq)
	}

	fn sequenced(&self, session_id: Option<&str>, mut frame: Value) -> Value {
		let seq = self.next_seq(session_id);
		if let (Some(seq), Some(object)) = (seq, frame.as_object_mut()) {
			object.insert("seq".to_owned(), json!(seq));
		}
		frame
	}

	fn member_of(&self, session_id: Option<&str>) -> Option<(u64, u64)> {
		let sessions = self.sessions.lock();
		let session = sessions.get(session_id?)?;
		Some((session.user_id?, session.channel_id?))
	}

	fn speaker(&self, ssrc: u32) -> Option<(u64, u64)> {
		let sessions = self.sessions.lock();
		let session = sessions.values().find(|session| session.owns(ssrc))?;
		Some((session.user_id?, session.channel_id?))
	}

	fn post(&self, mail: Mail) {
		for letter in mail {
			let _ = self.mail.send(letter);
		}
	}

	fn in_room(&self, channel_id: u64, act: impl FnOnce(&mut Harness) -> dave::Result<()>) {
		let mail = self.rooms.lock().run(channel_id, act);
		self.post(mail);
	}

	fn join_dave(&self, peer: &Peer) {
		if let Some((user_id, channel_id)) = self.member_of(peer.session_id.as_deref()) {
			let mail = self.rooms.lock().join_external(channel_id, user_id);
			self.post(mail);
		}
	}

	fn deliver_dave(&self, peer: &Peer, message: Wire) {
		if let Some((user_id, channel_id)) = self.member_of(peer.session_id.as_deref()) {
			let mail = self.rooms.lock().deliver(channel_id, user_id, message);
			self.post(mail);
		}
	}

	fn start_cameras(self: &Arc<Self>, peer: &Peer) {
		let (Some(cameras), Some((_, channel_id))) =
			(&self.cameras, self.member_of(peer.session_id.as_deref()))
		else {
			return;
		};
		cameras.start(&VoiceControl(self.clone()), channel_id);
	}

	fn in_channel(&self, peer: &Peer, channel: Option<u64>) -> bool {
		channel.is_none_or(|channel| {
			self.member_of(peer.session_id.as_deref())
				.is_some_and(|(_, joined)| joined == channel)
		})
	}

	fn open_dave(&self, ssrc: u32, payload: &[u8]) -> Option<Vec<u8>> {
		let (user_id, channel_id) = self.speaker(ssrc)?;
		self.rooms.lock().open(channel_id, user_id, payload)
	}

	fn open_video(&self, ssrc: u32, frame: &[u8]) -> Option<Vec<u8>> {
		let (user_id, channel_id) = self.speaker(ssrc)?;
		self.rooms.lock().open_video(channel_id, user_id, frame)
	}

	fn audio_ssrc_of(&self, ssrc: u32) -> Option<u32> {
		self.sessions
			.lock()
			.values()
			.find(|session| session.owns(ssrc))
			.map(|session| session.ssrc)
	}
}

#[derive(Clone)]
pub struct VoiceControl(Arc<Voice>);

impl VoiceControl {
	fn shutdown(&self) -> watch::Receiver<bool> {
		self.0.shutdown.clone()
	}

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

	pub fn hold_echo(&self) {
		self.0.echo_held.store(true, Ordering::Relaxed);
	}

	pub fn release_echo(&self) {
		self.0.echo_held.store(false, Ordering::Relaxed);
		self.0.echo_released.notify_one();
	}

	pub fn set_dave_version(&self, version: u8) {
		self.0.settings.lock().dave_version = version;
	}

	pub fn add_member(&self, channel_id: u64, user_id: u64) {
		self.0.in_room(channel_id, |room| room.join(user_id));
	}

	pub fn remove_member(&self, channel_id: u64, user_id: u64) {
		self.0.in_room(channel_id, |room| room.leave(user_id));
	}

	pub fn downgrade_dave(&self, channel_id: u64) {
		self.0.in_room(channel_id, Harness::downgrade);
	}

	pub fn upgrade_dave(&self, channel_id: u64) {
		self.0.in_room(channel_id, Harness::upgrade);
	}

	pub fn send_bogus_commit(&self, channel_id: u64, user_id: u64) {
		self.0.in_room(channel_id, |room| room.bogus_commit(user_id));
	}

	pub fn dave_epoch(&self, channel_id: u64) -> Option<u64> {
		self.0
			.rooms
			.lock()
			.get(channel_id)
			.map(|room| room.gateway.epoch())
	}

	pub fn dave_group(&self, channel_id: u64) -> Vec<u64> {
		self.0
			.rooms
			.lock()
			.get(channel_id)
			.map(|room| room.gateway.established().to_vec())
			.unwrap_or_default()
	}

	pub fn member_epoch(&self, channel_id: u64, user_id: u64) -> Option<u64> {
		self.0
			.rooms
			.lock()
			.get(channel_id)?
			.member(user_id)?
			.session
			.epoch()
	}

	pub fn send_as(&self, channel_id: u64, user_id: u64, ssrc: u32, sequence: u16, frame: &[u8]) -> usize {
		let Some(payload) = self.0.rooms.lock().seal(channel_id, user_id, frame) else {
			return 0;
		};
		let timestamp = u32::from(sequence).wrapping_mul(echo::OPUS_FRAME_SAMPLES);
		let mut header = vec![RTP_VERSION, OPUS_PAYLOAD_TYPE];
		header.extend_from_slice(&sequence.to_be_bytes());
		header.extend_from_slice(&timestamp.to_be_bytes());
		header.extend_from_slice(&ssrc.to_be_bytes());
		self.send_rtp(channel_id, &header, &payload)
	}

	pub fn seal_video(&self, channel_id: u64, user_id: u64, frame: &[u8]) -> Option<Vec<u8>> {
		self.0.rooms.lock().seal_video(channel_id, user_id, frame)
	}

	pub fn send_rtp(&self, channel_id: u64, header: &[u8], payload: &[u8]) -> usize {
		let voice = &self.0;
		let targets: Vec<(SocketAddr, String)> = {
			let sessions = voice.sessions.lock();
			let clients = voice.clients.lock();
			let modes = voice.modes.lock();
			sessions
				.values()
				.filter(|session| session.channel_id == Some(channel_id))
				.filter_map(|session| Some((*clients.get(&session.ssrc)?, modes.get(&session.ssrc)?.clone())))
				.collect()
		};
		let mut sent = 0;
		for (address, mode) in targets {
			let counter = voice.media_counter.fetch_add(1, Ordering::Relaxed).to_be_bytes();
			let Some(packet) = echo::seal_packet(&mode, &voice.key, &counter, header, payload) else {
				continue;
			};
			if voice.udp.try_send_to(&packet, address).is_ok() {
				sent += 1;
			}
		}
		sent
	}

	pub fn video_as(&self, user_id: u64, audio_ssrc: u32, video_ssrc: u32) {
		let frame = json!({
			"op": 12,
			"d": {
				"user_id": user_id.to_string(),
				"audio_ssrc": audio_ssrc,
				"video_ssrc": video_ssrc,
				"streams": [{
					"ssrc": video_ssrc,
					"rtx_ssrc": video_ssrc.wrapping_add(1),
					"rid": "100",
					"quality": 100,
					"max_resolution": { "type": "fixed", "width": CAMERA_WIDTH, "height": CAMERA_HEIGHT },
					"max_framerate": CAMERA_FPS,
					"active": true,
				}],
			},
		});
		let _ = self.0.broadcasts.send((SCRIPTED, None, frame));
	}

	pub fn video_off_as(&self, user_id: u64, audio_ssrc: u32) {
		let frame = json!({
			"op": 12,
			"d": {
				"user_id": user_id.to_string(),
				"audio_ssrc": audio_ssrc,
				"video_ssrc": 0,
				"streams": [],
			},
		});
		let _ = self.0.broadcasts.send((SCRIPTED, None, frame));
	}

	pub fn sink_wants(&self, wants: Value) {
		let frame = json!({ "op": 15, "d": wants });
		let _ = self.0.broadcasts.send((SCRIPTED, None, frame));
	}

	pub fn request_keyframe(&self, media_ssrc: u32) -> bool {
		let voice = &self.0;
		let Some(audio_ssrc) = voice.audio_ssrc_of(media_ssrc) else {
			return false;
		};
		let (Some(address), Some(mode)) = (
			voice.clients.lock().get(&audio_ssrc).copied(),
			voice.modes.lock().get(&audio_ssrc).cloned(),
		) else {
			return false;
		};
		let mut header = vec![
			RTCP_FEEDBACK_VERSION | PICTURE_LOSS_FORMAT,
			PAYLOAD_SPECIFIC_FEEDBACK,
		];
		header.extend_from_slice(&PICTURE_LOSS_WORDS.to_be_bytes());
		header.extend_from_slice(&0u32.to_be_bytes());
		let counter = voice.media_counter.fetch_add(1, Ordering::Relaxed).to_be_bytes();
		echo::seal_packet(&mode, &voice.key, &counter, &header, &media_ssrc.to_be_bytes())
			.is_some_and(|packet| voice.udp.try_send_to(&packet, address).is_ok())
	}

	pub fn connect_as(&self, user_id: u64) {
		let frame = json!({ "op": 11, "d": { "user_ids": [user_id.to_string()] } });
		let _ = self.0.broadcasts.send((SCRIPTED, None, frame));
	}

	pub fn disconnect_as(&self, user_id: u64) {
		let frame = json!({ "op": 13, "d": { "user_id": user_id.to_string() } });
		let _ = self.0.broadcasts.send((SCRIPTED, None, frame));
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
		let _ = self.0.broadcasts.send((SCRIPTED, None, frame));
	}
}

pub struct VoiceEndpoint {
	pub url: String,
	pub control: VoiceControl,
	pub tasks: Vec<JoinHandle<()>>,
}

pub async fn spawn(shutdown: watch::Receiver<bool>, cameras: Option<Cameras>) -> io::Result<VoiceEndpoint> {
	let socket = Arc::new(UdpSocket::bind(("127.0.0.1", 0)).await?);
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
			dave_version: 0,
		}),
		sessions: Mutex::new(HashMap::new()),
		modes: Mutex::new(HashMap::new()),
		log: Mutex::new(VoiceRecording::default()),
		revision: watch::Sender::new(0),
		shutdown: shutdown.clone(),
		echo_held: AtomicBool::new(false),
		held_echo: Mutex::new(Vec::new()),
		echo_released: Notify::new(),
		udp: socket.clone(),
		clients: Mutex::new(HashMap::new()),
		media_counter: AtomicU32::new(0),
		rooms: Mutex::new(Rooms::default()),
		mail: broadcast::channel(MAIL_CAPACITY).0,
		cameras,
		streams: Mutex::new(stream::Streams::default()),
		assembler: Mutex::new(Assembler::default()),
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
	let mut mail = voice.mail.subscribe();
	let mut shutdown = voice.shutdown.clone();
	loop {
		let flow = tokio::select! {
			incoming = socket.recv() => match incoming {
				Some(Ok(Message::Text(text))) => on_text(&voice, &mut socket, &mut peer, text.as_str()).await,
				Some(Ok(Message::Binary(bytes))) => {
					on_binary(&voice, &peer, &bytes);
					Flow::Continue
				}
				Some(Ok(Message::Close(frame))) => {
					let code = frame.map(|frame| frame.code);
					voice.record(|log| log.closes.push((connection, code)));
					Flow::Stop
				}
				Some(Err(_)) | None => Flow::Stop,
				Some(Ok(_)) => Flow::Continue,
			},
			broadcast = broadcasts.recv() => match broadcast {
				Ok((origin, channel, frame)) if origin != peer.connection && peer.session_id.is_some() && voice.in_channel(&peer, channel) => {
					let frame = voice.sequenced(peer.session_id.as_deref(), frame);
					if send(&voice, &mut socket, &peer, frame).await { Flow::Continue } else { Flow::Stop }
				}
				Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => Flow::Continue,
				Err(broadcast::error::RecvError::Closed) => Flow::Stop,
			},
			letter = mail.recv() => match letter {
				Ok((channel_id, user_id, message)) if addressed(&voice, &peer, channel_id, user_id) => {
					if send_wire(&voice, &mut socket, &peer, message).await { Flow::Continue } else { Flow::Stop }
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

async fn on_text(voice: &Arc<Voice>, socket: &mut WebSocket, peer: &mut Peer, text: &str) -> Flow {
	let Ok(frame) = serde_json::from_str::<Value>(text) else {
		return Flow::Continue;
	};
	voice.record(|log| {
		log.frames
			.push(VoiceFrame::new(peer.connection, &frame, text.to_owned()))
	});
	let data = &frame["d"];
	let op = frame["op"].as_u64();
	let reply = match op {
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
		Some(12) => {
			relay_video(voice, peer, data);
			return Flow::Continue;
		}
		Some(7) => match resume(voice, peer, data) {
			Ok(resumed) => resumed,
			Err(code) => return Flow::Close(code),
		},
		Some(DAVE_TRANSITION_READY | DAVE_INVALID_COMMIT_WELCOME) => {
			voice.deliver_dave(peer, Wire::Text(text.to_owned()));
			return Flow::Continue;
		}
		_ => return Flow::Continue,
	};
	let dave_starts = op == Some(1) && reply["d"]["dave_protocol_version"].as_u64().unwrap_or_default() > 0;
	if !send(voice, socket, peer, reply).await {
		return Flow::Stop;
	}
	if dave_starts {
		voice.join_dave(peer);
	}
	if op == Some(1) {
		voice.start_cameras(peer);
		voice.announce_streams(peer);
	}
	Flow::Continue
}

fn addressed(voice: &Voice, peer: &Peer, channel_id: u64, user_id: u64) -> bool {
	voice
		.member_of(peer.session_id.as_deref())
		.is_some_and(|(user, channel)| user == user_id && channel == channel_id)
}

fn session_key(data: &Value) -> String {
	let server = parse_id(&data["server_id"]).unwrap_or_default();
	let session = data["session_id"].as_str().unwrap_or_default();
	format!("{server}:{session}")
}

fn on_binary(voice: &Voice, peer: &Peer, bytes: &[u8]) {
	let Some((&op, payload)) = bytes.split_first() else {
		return;
	};
	voice.record(|log| {
		log.frames
			.push(VoiceFrame::binary(peer.connection, op, None, payload))
	});
	voice.deliver_dave(peer, Wire::Binary(bytes.to_vec()));
}

async fn send_wire(voice: &Voice, socket: &mut WebSocket, peer: &Peer, message: Wire) -> bool {
	match message {
		Wire::Text(text) => match serde_json::from_str::<Value>(&text) {
			Ok(frame) => {
				let frame = voice.sequenced(peer.session_id.as_deref(), frame);
				send(voice, socket, peer, frame).await
			}
			Err(_) => true,
		},
		Wire::Binary(bytes) => {
			let Some((&op, payload)) = bytes.split_first() else {
				return true;
			};
			let seq = voice.next_seq(peer.session_id.as_deref()).unwrap_or_default();
			let mut framed = (seq as u16).to_be_bytes().to_vec();
			framed.extend_from_slice(&bytes);
			let sent = socket.send(Message::Binary(framed.into())).await.is_ok();
			if sent {
				voice.record(|log| {
					log.sent
						.push(VoiceFrame::binary(peer.connection, op, Some(seq), payload))
				});
			}
			sent
		}
	}
}

fn issued(token: &Value) -> bool {
	token.as_str().is_some_and(|token| {
		token
			.strip_prefix(VOICE_TOKEN)
			.is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
	})
}

fn identify(voice: &Voice, peer: &mut Peer, data: &Value) -> Result<Value, u16> {
	if !issued(&data["token"]) {
		return Err(AUTHENTICATION_FAILED);
	}
	let session_id = session_key(data);
	let offered: Vec<&Value> = data["streams"]
		.as_array()
		.map(|streams| streams.iter().take(MAX_OFFERED_STREAMS).collect())
		.unwrap_or_default();
	let stride = SSRC_STRIDE + SSRCS_PER_STREAM * offered.len() as u32;
	let ssrc = voice.next_ssrc.fetch_add(stride, Ordering::Relaxed);
	let streams: Vec<Value> = offered
		.iter()
		.enumerate()
		.map(|(index, offer)| {
			let video_ssrc = ssrc + SSRC_STRIDE + SSRCS_PER_STREAM * index as u32;
			json!({
				"type": "video",
				"ssrc": video_ssrc,
				"rtx_ssrc": video_ssrc + 1,
				"rid": offer["rid"],
				"quality": offer["quality"],
				"active": false,
			})
		})
		.collect();
	voice.sessions.lock().insert(
		session_id.clone(),
		Session {
			user_id: parse_id(&data["user_id"]),
			channel_id: parse_id(&data["channel_id"]),
			ssrc,
			video_ssrcs: streams
				.iter()
				.filter_map(|stream| stream["ssrc"].as_u64().and_then(|ssrc| u32::try_from(ssrc).ok()))
				.collect(),
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
			"streams": streams,
		},
	}))
}

fn select_protocol(voice: &Voice, peer: &Peer, data: &Value) -> Result<Value, u16> {
	let mode = data["data"]["mode"].as_str().unwrap_or_default();
	if !voice.settings.lock().modes.iter().any(|offered| offered == mode) {
		return Err(UNKNOWN_ENCRYPTION_MODE);
	}
	let (ssrc, video_ssrcs) = peer
		.session_id
		.as_ref()
		.and_then(|session_id| {
			voice
				.sessions
				.lock()
				.get(session_id)
				.map(|session| (session.ssrc, session.video_ssrcs.clone()))
		})
		.ok_or(SESSION_INVALID)?;
	{
		let mut modes = voice.modes.lock();
		for owned in std::iter::once(ssrc).chain(video_ssrcs) {
			modes.insert(owned, mode.to_owned());
		}
	}
	let dave_version = voice.settings.lock().dave_version;
	Ok(json!({
		"op": 4,
		"d": {
			"audio_codec": "opus",
			"video_codec": "H264",
			"media_session_id": format!("mock-media-{ssrc}"),
			"mode": mode,
			"secret_key": voice.key.to_vec(),
			"dave_protocol_version": dave_version,
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
	let channel = voice.member_of(Some(session_id)).map(|(_, channel)| channel);
	let _ = voice.broadcasts.send((peer.connection, channel, frame));
}

fn relay_video(voice: &Voice, peer: &Peer, data: &Value) {
	let Some((user_id, channel)) = voice.member_of(peer.session_id.as_deref()) else {
		return;
	};
	let mut state = data.clone();
	if let Some(object) = state.as_object_mut() {
		object.remove("rtx_ssrc");
		object.insert("user_id".to_owned(), json!(user_id.to_string()));
	}
	let _ = voice
		.broadcasts
		.send((peer.connection, Some(channel), json!({ "op": 12, "d": state })));
}

fn resume(voice: &Voice, peer: &mut Peer, data: &Value) -> Result<Value, u16> {
	if !issued(&data["token"]) {
		return Err(AUTHENTICATION_FAILED);
	}
	let session_id = session_key(data);
	if !voice.sessions.lock().contains_key(&session_id) {
		return Err(SESSION_INVALID);
	}
	peer.session_id = Some(session_id);
	Ok(json!({ "op": 9, "d": null }))
}
