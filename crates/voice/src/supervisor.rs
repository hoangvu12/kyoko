use std::sync::Arc;
use std::time::{Duration, Instant};

use dave::{DaveEvent, DaveSession, Outgoing};
use protocol::Snowflake;
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::error::{Result, VoiceError};
use crate::event::{StopReason, VoiceEvent};
use crate::media::{Media, SharedTransform};
use crate::shared::{Control, Shared};
use crate::signal::{Body, Credentials, Established, Exit, Frame, Link, establish, resume, speaking};
use crate::transport::Transport;
use crate::wire::{ClientDisconnect, ClientsConnect, Speaking, close, opcode};

const RESUME_ATTEMPTS: u32 = 5;
const FRESH_ATTEMPTS: u32 = 3;
const MAX_BACKOFF_SHIFT: u32 = 3;

enum Next {
	Resume,
	Fresh,
	Stop,
}

fn classify(code: Option<u16>) -> Next {
	match code {
		None | Some(close::SERVER_CRASHED) => Next::Resume,
		Some(close::SESSION_INVALID | close::SESSION_TIMEOUT) => Next::Fresh,
		Some(code) if code < 4000 => Next::Resume,
		Some(_) => Next::Stop,
	}
}

fn backoff(attempt: u32) -> Duration {
	Duration::from_secs(1 << attempt.min(MAX_BACKOFF_SHIFT))
}

struct MediaHandle(JoinHandle<()>);

impl Drop for MediaHandle {
	fn drop(&mut self) {
		self.0.abort();
	}
}

enum Drive {
	Shutdown,
	Exit(Exit),
}

enum Recovery {
	Recovered,
	Stopped(StopReason),
	Shutdown,
}

pub(crate) struct Signals {
	pub shutdown: watch::Receiver<bool>,
	pub control: mpsc::UnboundedSender<Control>,
	pub commands: mpsc::UnboundedReceiver<Control>,
}

pub(crate) struct Supervisor {
	credentials: Credentials,
	shared: Arc<Shared>,
	transform: SharedTransform,
	audio_io: bool,
	signals: Signals,
	link: Link,
	media: MediaHandle,
	ssrc: u32,
	speaking: bool,
	dave: DaveSession,
	outbox: Vec<Outgoing>,
}

impl Supervisor {
	pub fn start(
		credentials: Credentials,
		established: Established,
		shared: Arc<Shared>,
		transform: SharedTransform,
		dave: DaveSession,
		audio_io: bool,
		signals: Signals,
	) -> Result<Self> {
		let ssrc = established.transport.ssrc();
		let dave_version = established.dave_version;
		let media = spawn_media(
			established.transport,
			&shared,
			&signals.control,
			&transform,
			audio_io,
		)?;
		shared.hub.emit(VoiceEvent::Connected {
			ssrc,
			mode: established.mode,
		});
		let mut supervisor = Self {
			credentials,
			shared,
			transform,
			audio_io,
			signals,
			link: established.link,
			media,
			ssrc,
			speaking: false,
			dave,
			outbox: Vec::new(),
		};
		supervisor.outbox = supervisor.dave.on_protocol_version(dave_version);
		supervisor.replay_backlog();
		Ok(supervisor)
	}

	pub async fn run(mut self) {
		let reason = self.serve().await;
		self.link.close().await;
		self.shared.engine.lock().take();
		self.shared.hub.emit(VoiceEvent::Stopped(reason));
	}

	async fn serve(&mut self) -> StopReason {
		loop {
			let exit = match self.drive().await {
				Drive::Shutdown => return StopReason::Left,
				Drive::Exit(exit) => exit,
			};
			let code = match exit {
				Exit::Closed(code) => code,
				Exit::Lost => None,
			};
			if matches!(classify(code), Next::Stop) {
				return StopReason::from_close(code);
			}
			match self.recover(code).await {
				Recovery::Recovered => {}
				Recovery::Stopped(reason) => return reason,
				Recovery::Shutdown => return StopReason::Left,
			}
		}
	}

	async fn drive(&mut self) -> Drive {
		loop {
			if let Err(exit) = self.flush_dave().await {
				return Drive::Exit(exit);
			}
			tokio::select! {
				frame = self.link.next() => match frame {
					Ok(frame) => self.on_frame(frame),
					Err(exit) => return Drive::Exit(exit),
				},
				command = self.signals.commands.recv() => match command {
					Some(Control::Speaking(now)) => {
						self.speaking = now;
						self.shared.speaking(self.credentials.user_id(), now);
						if let Err(exit) = self.link.send(opcode::SPEAKING, speaking(now, self.ssrc)).await {
							return Drive::Exit(exit);
						}
					}
					None => return Drive::Shutdown,
				},
				_ = self.signals.shutdown.changed() => return Drive::Shutdown,
			}
		}
	}

	fn replay_backlog(&mut self) {
		while let Some(frame) = self.link.backlog.pop_front() {
			self.on_frame(frame);
		}
	}

	fn on_frame(&mut self, frame: Frame) {
		match frame.body {
			Body::Json(data) => self.on_json(frame.op, data),
			Body::Binary(payload) => {
				let outgoing = self.dave.on_binary(frame.op, &payload);
				self.outbox.extend(outgoing);
			}
		}
	}

	fn on_json(&mut self, op: u8, data: Value) {
		match op {
			opcode::SPEAKING => {
				if let Ok(event) = serde_json::from_value::<Speaking>(data) {
					self.on_speaking(&event);
				}
			}
			opcode::CLIENTS_CONNECT => {
				if let Ok(event) = serde_json::from_value::<ClientsConnect>(data) {
					for user_id in event.user_ids {
						self.dave.user_joined(user_id.get());
						self.shared.hub.emit(VoiceEvent::UserJoined { user_id });
					}
				}
			}
			opcode::CLIENT_DISCONNECT => {
				if let Ok(event) = serde_json::from_value::<ClientDisconnect>(data) {
					self.dave.user_left(event.user_id.get());
					self.shared
						.ssrc_users
						.lock()
						.retain(|_, user| *user != event.user_id);
					self.shared.hub.emit(VoiceEvent::UserLeft {
						user_id: event.user_id,
					});
				}
			}
			opcode::DAVE_PREPARE_TRANSITION
			| opcode::DAVE_EXECUTE_TRANSITION
			| opcode::DAVE_PREPARE_EPOCH => {
				let outgoing = self.dave.on_json(op, &data);
				self.outbox.extend(outgoing);
			}
			opcode::HEARTBEAT_ACK => self.on_ack(),
			_ => {}
		}
	}

	async fn flush_dave(&mut self) -> std::result::Result<(), Exit> {
		for event in self.dave.take_events() {
			self.shared.hub.emit(match event {
				DaveEvent::Epoch {
					epoch,
					version,
					members,
				} => VoiceEvent::Dave {
					epoch,
					version,
					members: members.into_iter().map(Snowflake).collect(),
				},
				DaveEvent::Failure(reason) => VoiceEvent::DaveFailure(reason),
			});
		}
		for outgoing in std::mem::take(&mut self.outbox) {
			self.link.send_wire(outgoing.encode()).await?;
		}
		Ok(())
	}

	fn on_speaking(&mut self, event: &Speaking) {
		let Some(user_id) = event.user_id else {
			return;
		};
		if let Some(ssrc) = event.ssrc {
			self.shared.ssrc_users.lock().insert(ssrc, user_id);
		}
		self.dave.user_joined(user_id.get());
		if event.speaking.unwrap_or_default() == 0 {
			self.shared.hushed(user_id);
		} else {
			self.shared.announced(user_id, Instant::now());
		}
	}

	fn on_ack(&self) {
		let Some(ping_ms) = self.link.ping_ms else {
			return;
		};
		let counters = &self.shared.counters;
		counters.set_ping(ping_ms);
		self.shared.hub.emit(VoiceEvent::Stats {
			ping_ms,
			packets_lost: counters.lost.load(std::sync::atomic::Ordering::Relaxed),
			jitter_ms: counters.jitter_ms.load(),
		});
	}

	async fn recover(&mut self, code: Option<u16>) -> Recovery {
		let mut code = code;
		if matches!(classify(code), Next::Resume) {
			self.shared
				.hub
				.emit(VoiceEvent::Reconnecting { code, resume: true });
			for attempt in 0..RESUME_ATTEMPTS {
				if !self.pause(backoff(attempt)).await {
					return Recovery::Shutdown;
				}
				match resume(&self.credentials, self.link.last_seq).await {
					Ok(link) => {
						self.link = link;
						return self.after_resume().await;
					}
					Err(VoiceError::Closed(closed)) => {
						code = Some(closed);
						match classify(code) {
							Next::Resume => {}
							Next::Fresh => break,
							Next::Stop => return Recovery::Stopped(StopReason::from_close(code)),
						}
					}
					Err(error) => debug!(%error, "voice resume attempt failed"),
				}
				if attempt + 1 == RESUME_ATTEMPTS {
					return Recovery::Stopped(StopReason::Unreachable);
				}
			}
		}
		self.shared
			.hub
			.emit(VoiceEvent::Reconnecting { code, resume: false });
		for attempt in 0..FRESH_ATTEMPTS {
			if !self.pause(backoff(attempt)).await {
				return Recovery::Shutdown;
			}
			match establish(&self.credentials).await {
				Ok(established) => return self.adopt(established),
				Err(VoiceError::Closed(closed)) => {
					return Recovery::Stopped(StopReason::from_close(Some(closed)));
				}
				Err(error) => debug!(%error, "voice reconnect attempt failed"),
			}
		}
		Recovery::Stopped(StopReason::Unreachable)
	}

	async fn after_resume(&mut self) -> Recovery {
		if self
			.link
			.send(opcode::SPEAKING, speaking(self.speaking, self.ssrc))
			.await
			.is_err()
		{
			return Recovery::Stopped(StopReason::Unreachable);
		}
		self.replay_backlog();
		self.shared.hub.emit(VoiceEvent::Resumed);
		Recovery::Recovered
	}

	fn adopt(&mut self, established: Established) -> Recovery {
		self.shared.ssrc_users.lock().clear();
		self.shared.hush_everyone();
		self.ssrc = established.transport.ssrc();
		if std::mem::take(&mut self.speaking) {
			self.shared.speaking(self.credentials.user_id(), false);
		}
		match spawn_media(
			established.transport,
			&self.shared,
			&self.signals.control,
			&self.transform,
			self.audio_io,
		) {
			Ok(media) => self.media = media,
			Err(error) => {
				warn!(%error, "failed to restart voice media");
				return Recovery::Stopped(StopReason::Unreachable);
			}
		}
		self.link = established.link;
		self.outbox = self.dave.on_protocol_version(established.dave_version);
		self.replay_backlog();
		self.shared.hub.emit(VoiceEvent::Connected {
			ssrc: self.ssrc,
			mode: established.mode,
		});
		Recovery::Recovered
	}

	async fn pause(&mut self, duration: Duration) -> bool {
		tokio::select! {
			() = sleep(duration) => true,
			_ = self.signals.shutdown.changed() => false,
		}
	}
}

fn spawn_media(
	transport: Arc<dyn Transport>,
	shared: &Arc<Shared>,
	control: &mpsc::UnboundedSender<Control>,
	transform: &SharedTransform,
	audio_io: bool,
) -> Result<MediaHandle> {
	let media = Media::new(
		transport,
		shared.clone(),
		control.clone(),
		transform.clone(),
		audio_io,
	)?;
	Ok(MediaHandle(tokio::spawn(media.run())))
}
