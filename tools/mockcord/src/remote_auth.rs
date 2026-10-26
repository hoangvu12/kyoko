use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use axum::Json;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use parking_lot::Mutex;
use rsa::pkcs8::DecodePublicKey;
use rsa::rand_core::{OsRng, RngCore};
use rsa::{Oaep, RsaPublicKey};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::error::ApiError;
use crate::record::{Headers, header_pairs};
use crate::state::Shared;

pub const PATH: &str = "/remote-auth";
const NONCE_BYTES: usize = 32;
const TICKET_BYTES: usize = 24;
const CLOSE_NORMAL: u16 = 1000;
const CLOSE_GOING_AWAY: u16 = 1001;
const CLOSE_DECODE_ERROR: u16 = 4001;
const CLOSE_TIMED_OUT: u16 = 4003;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
	Scan,
	Confirm,
	Cancel,
	Expire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phone {
	Waiting,
	Scans,
	SignsIn,
	Expires,
}

impl Phone {
	fn named(name: Option<&str>) -> Phone {
		match name {
			Some("scan") => Phone::Scans,
			Some("login") => Phone::SignsIn,
			Some("expire") => Phone::Expires,
			_ => Phone::Waiting,
		}
	}

	fn steps(self) -> &'static [Step] {
		match self {
			Phone::Waiting => &[],
			Phone::Scans => &[Step::Scan],
			Phone::SignsIn => &[Step::Scan, Step::Confirm],
			Phone::Expires => &[Step::Expire],
		}
	}
}

#[derive(Default)]
pub struct RemoteAuth {
	next_id: AtomicU64,
	phones: Mutex<HashMap<u64, UnboundedSender<Step>>>,
	tickets: Mutex<HashMap<String, RsaPublicKey>>,
	connections: Mutex<Vec<Headers>>,
	garble_next_nonce: AtomicBool,
}

impl RemoteAuth {
	fn step_all(&self, step: Step) {
		self.phones.lock().retain(|_, phone| phone.send(step).is_ok());
	}

	pub fn scan(&self) {
		self.step_all(Step::Scan);
	}

	pub fn confirm(&self) {
		self.step_all(Step::Confirm);
	}

	pub fn cancel(&self) {
		self.step_all(Step::Cancel);
	}

	pub fn expire(&self) {
		self.step_all(Step::Expire);
	}

	pub fn garble_next_nonce(&self) {
		self.garble_next_nonce.store(true, Ordering::SeqCst);
	}

	pub fn connections(&self) -> Vec<Headers> {
		self.connections.lock().clone()
	}

	pub fn waiting_phones(&self) -> usize {
		self.phones.lock().len()
	}
}

#[derive(Deserialize)]
pub struct RemoteAuthQuery {
	phone: Option<String>,
}

pub async fn upgrade(
	State(shared): State<Arc<Shared>>,
	Query(query): Query<RemoteAuthQuery>,
	headers: HeaderMap,
	socket: WebSocketUpgrade,
) -> Response {
	shared.remote_auth.connections.lock().push(header_pairs(&headers));
	let phone = Phone::named(query.phone.as_deref());
	socket.on_upgrade(move |socket| Link::new(shared, socket, phone).run())
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Incoming {
	Init {
		encoded_public_key: String,
	},
	Heartbeat,
	NonceProof {
		nonce: String,
	},
	#[serde(other)]
	Unknown,
}

struct Link {
	shared: Arc<Shared>,
	socket: WebSocket,
	phone: Phone,
	id: u64,
	key: Option<RsaPublicKey>,
	nonce: Option<String>,
}

fn random_bytes(count: usize) -> Vec<u8> {
	let mut bytes = vec![0; count];
	OsRng.fill_bytes(&mut bytes);
	bytes
}

fn encrypt(key: &RsaPublicKey, plain: &[u8]) -> Option<String> {
	key.encrypt(&mut OsRng, Oaep::new::<Sha256>(), plain)
		.ok()
		.map(|cipher| STANDARD.encode(cipher))
}

fn fingerprint(der: &[u8]) -> String {
	URL_SAFE_NO_PAD.encode(Sha256::digest(der))
}

impl Link {
	fn new(shared: Arc<Shared>, socket: WebSocket, phone: Phone) -> Self {
		let id = shared.remote_auth.next_id.fetch_add(1, Ordering::SeqCst);
		Self {
			shared,
			socket,
			phone,
			id,
			key: None,
			nonce: None,
		}
	}

	async fn run(mut self) {
		let timeout = Duration::from_millis(self.shared.config.remote_auth_timeout_ms);
		let hello = json!({
			"op": "hello",
			"heartbeat_interval": self.shared.config.remote_auth_heartbeat_ms,
			"timeout_ms": self.shared.config.remote_auth_timeout_ms,
		});
		if !self.send(&hello).await {
			return;
		}
		let (steps, mut queued) = unbounded_channel();
		let mut shutdown = self.shared.shutdown.clone();
		let deadline = tokio::time::sleep(timeout);
		tokio::pin!(deadline);
		loop {
			tokio::select! {
				incoming = self.socket.recv() => match incoming {
					Some(Ok(Message::Text(text))) => {
						if !self.handle(text.as_str(), &steps).await {
							break;
						}
					}
					Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
					_ => break,
				},
				step = queued.recv() => match step {
					Some(step) => {
						if !self.take(step).await {
							break;
						}
					}
					None => break,
				},
				() = &mut deadline => {
					self.close(CLOSE_TIMED_OUT, "Session timed out").await;
					break;
				}
				_ = shutdown.changed() => {
					self.close(CLOSE_GOING_AWAY, "Server shutting down").await;
					break;
				}
			}
		}
		self.shared.remote_auth.phones.lock().remove(&self.id);
	}

	async fn send(&mut self, message: &Value) -> bool {
		self.socket
			.send(Message::Text(message.to_string().into()))
			.await
			.is_ok()
	}

	async fn close(&mut self, code: u16, reason: &'static str) {
		let frame = CloseFrame {
			code,
			reason: reason.into(),
		};
		let _ = self.socket.send(Message::Close(Some(frame))).await;
	}

	async fn handle(&mut self, text: &str, steps: &UnboundedSender<Step>) -> bool {
		let Ok(incoming) = serde_json::from_str::<Incoming>(text) else {
			self.close(CLOSE_DECODE_ERROR, "Decode error").await;
			return false;
		};
		match incoming {
			Incoming::Heartbeat => self.send(&json!({ "op": "heartbeat_ack" })).await,
			Incoming::Init { encoded_public_key } => self.init(&encoded_public_key).await,
			Incoming::NonceProof { nonce } => self.prove(&nonce, steps).await,
			Incoming::Unknown => true,
		}
	}

	async fn init(&mut self, encoded: &str) -> bool {
		let key = STANDARD
			.decode(encoded)
			.ok()
			.and_then(|der| RsaPublicKey::from_public_key_der(&der).ok());
		let Some(key) = key else {
			self.close(CLOSE_DECODE_ERROR, "Invalid public key").await;
			return false;
		};
		let nonce = random_bytes(NONCE_BYTES);
		let Some(encrypted_nonce) = encrypt(&key, &nonce) else {
			self.close(CLOSE_DECODE_ERROR, "Invalid public key").await;
			return false;
		};
		let expected = if self
			.shared
			.remote_auth
			.garble_next_nonce
			.swap(false, Ordering::SeqCst)
		{
			random_bytes(NONCE_BYTES)
		} else {
			nonce
		};
		self.nonce = Some(URL_SAFE_NO_PAD.encode(expected));
		self.key = Some(key);
		self.send(&json!({ "op": "nonce_proof", "encrypted_nonce": encrypted_nonce }))
			.await
	}

	async fn prove(&mut self, nonce: &str, steps: &UnboundedSender<Step>) -> bool {
		let proved = self.nonce.take().is_some_and(|expected| expected == nonce);
		let der = self.key.as_ref().and_then(|key| {
			rsa::pkcs8::EncodePublicKey::to_public_key_der(key)
				.ok()
				.map(|der| der.into_vec())
		});
		let (true, Some(der)) = (proved, der) else {
			self.close(CLOSE_DECODE_ERROR, "Invalid nonce proof").await;
			return false;
		};
		self.shared
			.remote_auth
			.phones
			.lock()
			.insert(self.id, steps.clone());
		let shown = self
			.send(&json!({ "op": "pending_remote_init", "fingerprint": fingerprint(&der) }))
			.await;
		for step in self.phone.steps() {
			let _ = steps.send(*step);
		}
		shown
	}

	async fn take(&mut self, step: Step) -> bool {
		let Some(key) = self.key.clone() else {
			return true;
		};
		match step {
			Step::Scan => {
				let user = self.shared.store.lock().current_user.clone();
				let payload = format!(
					"{}:{}:{}:{}",
					user.id,
					user.discriminator.as_deref().unwrap_or("0"),
					user.avatar.as_deref().unwrap_or("0"),
					user.username
				);
				match encrypt(&key, payload.as_bytes()) {
					Some(encrypted) => {
						self.send(&json!({ "op": "pending_ticket", "encrypted_user_payload": encrypted }))
							.await
					}
					None => false,
				}
			}
			Step::Confirm => {
				let ticket = URL_SAFE_NO_PAD.encode(random_bytes(TICKET_BYTES));
				self.shared.remote_auth.tickets.lock().insert(ticket.clone(), key);
				let _ = self
					.send(&json!({ "op": "pending_login", "ticket": ticket }))
					.await;
				self.close(CLOSE_NORMAL, "Finished").await;
				false
			}
			Step::Cancel => {
				let _ = self.send(&json!({ "op": "cancel" })).await;
				self.close(CLOSE_NORMAL, "Cancelled").await;
				false
			}
			Step::Expire => {
				self.close(CLOSE_TIMED_OUT, "Session timed out").await;
				false
			}
		}
	}
}

#[derive(Deserialize)]
pub struct Exchange {
	ticket: Option<String>,
}

pub async fn exchange(State(shared): State<Arc<Shared>>, Json(body): Json<Exchange>) -> Response {
	let key = body
		.ticket
		.and_then(|ticket| shared.remote_auth.tickets.lock().remove(&ticket));
	let Some(key) = key else {
		return ApiError::invalid_body("Invalid Form Body").into_response();
	};
	let token = shared.current_token();
	match encrypt(&key, token.as_bytes()) {
		Some(encrypted_token) => Json(json!({ "encrypted_token": encrypted_token })).into_response(),
		None => ApiError::invalid_body("Invalid Form Body").into_response(),
	}
}
