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
use crate::video::hub::RemoteVideo;
use crate::video::wants::sink_wants;
use crate::wire::{
	ClientDisconnect, ClientsConnect, Resolution, Speaking, StreamAnnouncement, VideoAnnouncement,
	VideoState, close, opcode,
};

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
	sound: bool,
	dave: DaveSession,
	outbox: Vec<Outgoing>,
	sent_wants: Option<Value>,
	camera: bool,
	announce: bool,
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
		shared.video.outgoing.set_layers(established.layers);
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
			sound: false,
			dave,
			outbox: Vec::new(),
			sent_wants: Some(handshake_wants()),
			camera: false,
			announce: false,
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
			if let Err(exit) = self.flush_wants().await {
				return Drive::Exit(exit);
			}
			if let Err(exit) = self.flush_camera().await {
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
						if let Err(exit) = self.send_speaking().await {
							return Drive::Exit(exit);
						}
					}
					Some(Control::SoundShare(now)) => {
						self.sound = now;
						if let Err(exit) = self.send_speaking().await {
							return Drive::Exit(exit);
						}
					}
					Some(Control::VideoWants) => {}
					Some(Control::CameraOn) => {
						self.camera = true;
						self.announce = true;
					}
					Some(Control::CameraOff) => {
						if std::mem::take(&mut self.camera) {
							self.announce = false;
							self.shared.video.outgoing.set_sending(false);
							if let Err(exit) = self.link.send(opcode::VIDEO, self.cleared_video()).await {
								return Drive::Exit(exit);
							}
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
			opcode::VIDEO => {
				if let Ok(event) = serde_json::from_value::<VideoAnnouncement>(data) {
					self.on_video(&event);
				}
			}
			opcode::CLIENT_DISCONNECT => {
				if let Ok(event) = serde_json::from_value::<ClientDisconnect>(data) {
					self.end_video(event.user_id);
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
			opcode::MEDIA_SINK_WANTS => self.on_sink_wants(&data),
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

	fn on_video(&mut self, event: &VideoAnnouncement) {
		let Some(user_id) = event.user_id else {
			return;
		};
		if user_id == self.credentials.user_id() {
			return;
		}
		let primary = event.video_ssrc.filter(|ssrc| *ssrc != 0);
		let mut ssrcs: Vec<u32> = event
			.streams
			.iter()
			.filter(|stream| stream.active != Some(false))
			.filter_map(|stream| stream.ssrc)
			.chain(primary)
			.filter(|ssrc| *ssrc != 0)
			.collect();
		ssrcs.sort_unstable();
		ssrcs.dedup();
		{
			let mut users = self.shared.ssrc_users.lock();
			for ssrc in event.audio_ssrc.into_iter().chain(ssrcs.iter().copied()) {
				if ssrc != 0 {
					users.insert(ssrc, user_id);
				}
			}
		}
		self.dave.user_joined(user_id.get());
		let Some(primary) = primary.or_else(|| ssrcs.first().copied()) else {
			self.end_video(user_id);
			return;
		};
		let started = self
			.shared
			.video
			.remote
			.lock()
			.insert(user_id, RemoteVideo { primary, ssrcs })
			.is_none();
		if started {
			self.shared.hub.emit(VoiceEvent::VideoStarted { user_id });
		}
	}

	fn end_video(&mut self, user_id: Snowflake) {
		if self.shared.video.remote.lock().remove(&user_id).is_none() {
			return;
		}
		if let Some(sink) = self.shared.video.sink() {
			sink.ended(user_id);
		}
		self.shared.hub.emit(VoiceEvent::VideoStopped { user_id });
	}

	fn end_all_video(&mut self) {
		let users: Vec<Snowflake> = self.shared.video.remote.lock().keys().copied().collect();
		for user_id in users {
			self.end_video(user_id);
		}
	}

	async fn flush_wants(&mut self) -> std::result::Result<(), Exit> {
		let wants = sink_wants(&self.shared.video.remote.lock(), &self.shared.video.wants.lock());
		if self.sent_wants.as_ref() == Some(&wants) {
			return Ok(());
		}
		self.link.send(opcode::MEDIA_SINK_WANTS, &wants).await?;
		self.sent_wants = Some(wants);
		Ok(())
	}

	async fn send_speaking(&mut self) -> std::result::Result<(), Exit> {
		self.link
			.send(opcode::SPEAKING, speaking(self.speaking, self.sound, self.ssrc))
			.await
	}

	async fn flush_camera(&mut self) -> std::result::Result<(), Exit> {
		if !std::mem::take(&mut self.announce) || !self.camera {
			return Ok(());
		}
		self.link.send(opcode::VIDEO, self.video_state()).await?;
		self.shared.video.outgoing.set_sending(true);
		Ok(())
	}

	fn video_state(&self) -> VideoState {
		let outgoing = &self.shared.video.outgoing;
		let quality = outgoing.quality();
		let layers = outgoing.layers();
		let streams: Vec<StreamAnnouncement> = layers
			.iter()
			.map(|state| {
				let layer = quality.layer(state.layer.quality);
				StreamAnnouncement {
					kind: self.credentials.video_kind(),
					rid: state.layer.rid.clone(),
					ssrc: state.layer.ssrc,
					active: state.wanted(),
					quality: state.layer.quality,
					rtx_ssrc: state.layer.rtx_ssrc,
					max_bitrate: layer.bitrate,
					max_framerate: layer.fps,
					max_resolution: Resolution {
						kind: "fixed",
						width: layer.width,
						height: layer.height,
					},
				}
			})
			.collect();
		let primary = layers.first();
		VideoState {
			audio_ssrc: self.ssrc,
			video_ssrc: primary.map_or(0, |state| state.layer.ssrc),
			rtx_ssrc: primary.map_or(0, |state| state.layer.rtx_ssrc),
			streams,
		}
	}

	fn cleared_video(&self) -> VideoState {
		VideoState {
			audio_ssrc: self.ssrc,
			video_ssrc: 0,
			rtx_ssrc: 0,
			streams: Vec::new(),
		}
	}

	fn on_sink_wants(&mut self, data: &Value) {
		let Some(wants) = data.as_object() else {
			return;
		};
		let level = |ssrc: u32| {
			wants
				.get(&ssrc.to_string())
				.or_else(|| wants.get("any"))
				.and_then(Value::as_u64)
		};
		let mut changed = false;
		for state in self.shared.video.outgoing.layers() {
			if let Some(level) = level(state.layer.ssrc) {
				changed |= state.set_wanted(level > 0);
			}
		}
		if changed && self.camera {
			self.announce = true;
		}
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
			.send(opcode::SPEAKING, speaking(self.speaking, self.sound, self.ssrc))
			.await
			.is_err()
		{
			return Recovery::Stopped(StopReason::Unreachable);
		}
		self.sent_wants = None;
		self.announce = self.camera;
		self.replay_backlog();
		self.shared.hub.emit(VoiceEvent::Resumed);
		Recovery::Recovered
	}

	fn adopt(&mut self, established: Established) -> Recovery {
		self.end_all_video();
		self.sent_wants = Some(handshake_wants());
		self.shared.ssrc_users.lock().clear();
		self.shared.hush_everyone();
		self.ssrc = established.transport.ssrc();
		if std::mem::take(&mut self.speaking) {
			self.shared.speaking(self.credentials.user_id(), false);
		}
		self.sound = false;
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
		self.shared.video.outgoing.set_sending(false);
		self.shared.video.outgoing.set_layers(established.layers);
		self.announce = self.camera;
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

fn handshake_wants() -> Value {
	sink_wants(&std::collections::HashMap::new(), &[])
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
