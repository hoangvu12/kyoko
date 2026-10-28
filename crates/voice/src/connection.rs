use std::sync::Arc;
use std::sync::atomic::Ordering;

use dave::{DaveSession, DaveStats};
use net::Trust;
use parking_lot::Mutex;
use protocol::{Snowflake, Token};
use tokio::sync::{mpsc, watch};

use crate::audio::{AudioEngine, Rings};
use crate::codec::{MAX_BITRATE_BPS, MIN_BITRATE_BPS};
use crate::error::{Result, VoiceError};
use crate::event::VoiceEvent;
use crate::media::SharedTransform;
use crate::shared::{AudioDevicesInUse, Control, InputMode, MixedFrame, Shared, VoiceStats};
use crate::signal::{Browser, Credentials, establish};
use crate::supervisor::{Signals, Supervisor};
use crate::transform::DaveTransform;
use crate::video::{VideoSink, VideoStats, VideoWant};

const MAX_VOLUME: f32 = 2.0;
const DEFAULT_LOCALE: &str = "en-US";

#[derive(Debug)]
pub struct VoiceParams {
	pub guild_id: Option<Snowflake>,
	pub channel_id: Snowflake,
	pub user_id: Snowflake,
	pub session_id: String,
	pub token: Token,
	pub endpoint: String,
}

impl VoiceParams {
	pub fn server_id(&self) -> Snowflake {
		self.guild_id.unwrap_or(self.channel_id)
	}

	pub fn dave_group(&self, kind: ConnectionKind) -> u64 {
		match kind {
			ConnectionKind::Voice => self.channel_id.get(),
			ConnectionKind::Stream => self.server_id().get().saturating_sub(1),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionKind {
	#[default]
	Voice,
	Stream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioIo {
	Devices {
		input: Option<String>,
		output: Option<String>,
	},
	Playback {
		output: Option<String>,
	},
	Manual,
}

pub struct VoiceOptions {
	pub audio: AudioIo,
	pub trust: Trust,
	pub user_agent: String,
	pub locale: String,
	pub kind: ConnectionKind,
}

impl Default for VoiceOptions {
	fn default() -> Self {
		Self {
			audio: AudioIo::Devices {
				input: None,
				output: None,
			},
			trust: Trust::default(),
			user_agent: protocol::fingerprint::user_agent(),
			locale: DEFAULT_LOCALE.to_owned(),
			kind: ConnectionKind::Voice,
		}
	}
}

pub struct VoiceConnection {
	shared: Arc<Shared>,
	dave: Arc<DaveStats>,
	control: mpsc::UnboundedSender<Control>,
	shutdown: watch::Sender<bool>,
}

impl VoiceConnection {
	pub async fn connect(params: VoiceParams, options: VoiceOptions) -> Result<Self> {
		let browser = Browser {
			user_agent: &options.user_agent,
			locale: &options.locale,
			trust: &options.trust,
			kind: options.kind,
		};
		let group = params.dave_group(options.kind);
		let credentials = Credentials::new(
			[params.server_id(), params.channel_id, params.user_id],
			params.session_id,
			params.token,
			&params.endpoint,
			&browser,
		)?;
		let established = establish(&credentials).await?;
		let dave = DaveSession::new(params.user_id.get(), group)?;
		let dave_stats = dave.stats();
		let rings = Arc::new(Rings::new());
		let shared = Arc::new(Shared::new(rings.clone()));
		let audio_io = match options.audio {
			AudioIo::Devices { input, output } => {
				*shared.devices.lock() = AudioDevicesInUse {
					input: input.clone(),
					output: output.clone(),
				};
				let engine = tokio::task::spawn_blocking(move || AudioEngine::start(rings, input, output))
					.await
					.map_err(|error| VoiceError::Audio(error.to_string()))??;
				*shared.engine.lock() = Some(engine);
				true
			}
			AudioIo::Playback { output } => {
				shared.devices.lock().output.clone_from(&output);
				let engine = tokio::task::spawn_blocking(move || AudioEngine::playback(rings, output))
					.await
					.map_err(|error| VoiceError::Audio(error.to_string()))??;
				*shared.engine.lock() = Some(engine);
				true
			}
			AudioIo::Manual => false,
		};
		let transform: SharedTransform = Arc::new(Mutex::new(Box::new(DaveTransform::new(
			dave.crypto(),
			shared.clone(),
		))));
		let (shutdown, shutdown_receiver) = watch::channel(false);
		let (control, commands) = mpsc::unbounded_channel();
		let supervisor = Supervisor::start(
			credentials,
			established,
			shared.clone(),
			transform,
			dave,
			audio_io,
			Signals {
				shutdown: shutdown_receiver,
				control: control.clone(),
				commands,
			},
		)?;
		tokio::spawn(supervisor.run());
		Ok(Self {
			shared,
			dave: dave_stats,
			control,
			shutdown,
		})
	}

	pub fn events(&self) -> mpsc::Receiver<VoiceEvent> {
		self.shared.hub.subscribe()
	}

	pub fn ssrc(&self) -> Option<u32> {
		self.shared.hub.ssrc()
	}

	pub fn set_mute(&self, muted: bool) {
		self.shared.muted.store(muted, Ordering::Relaxed);
	}

	pub fn set_deaf(&self, deafened: bool) {
		self.shared.deafened.store(deafened, Ordering::Relaxed);
	}

	pub fn set_input_device(&self, id: Option<String>) {
		self.shared.devices.lock().input.clone_from(&id);
		self.shared.restart_capture.store(true, Ordering::Relaxed);
		self.shared.audio_restarts.fetch_add(1, Ordering::Relaxed);
		if let Some(engine) = self.shared.engine.lock().as_ref() {
			engine.set_input(id);
		}
	}

	pub fn set_output_device(&self, id: Option<String>) {
		self.shared.devices.lock().output.clone_from(&id);
		self.shared.audio_restarts.fetch_add(1, Ordering::Relaxed);
		if let Some(engine) = self.shared.engine.lock().as_ref() {
			engine.set_output(id);
		}
	}

	pub fn audio_devices(&self) -> AudioDevicesInUse {
		self.shared.devices.lock().clone()
	}

	pub fn audio_restarts(&self) -> u64 {
		self.shared.audio_restarts.load(Ordering::Relaxed)
	}

	pub fn set_user_volume(&self, user_id: Snowflake, volume: f32) {
		self.shared
			.user_volumes
			.lock()
			.insert(user_id, volume.clamp(0.0, MAX_VOLUME));
	}

	pub fn set_bitrate(&self, bps: u32) {
		self.shared
			.bitrate
			.store(bps.clamp(MIN_BITRATE_BPS, MAX_BITRATE_BPS), Ordering::Relaxed);
	}

	pub fn bitrate(&self) -> u32 {
		self.shared.applied_bitrate.load(Ordering::Relaxed)
	}

	pub fn user_volume(&self, user_id: Snowflake) -> f32 {
		self.shared
			.user_volumes
			.lock()
			.get(&user_id)
			.copied()
			.unwrap_or(1.0)
	}

	pub fn set_input_volume(&self, volume: f32) {
		self.shared.input_volume.store(volume.clamp(0.0, MAX_VOLUME));
	}

	pub fn set_output_volume(&self, volume: f32) {
		self.shared.output_volume.store(volume.clamp(0.0, MAX_VOLUME));
	}

	pub fn set_input_mode(&self, mode: InputMode) {
		self.shared.set_input_mode(mode);
	}

	pub fn set_vad_threshold_db(&self, threshold_db: f32) {
		self.shared.vad_threshold_db.store(threshold_db);
	}

	pub fn set_transmitting(&self, transmitting: bool) {
		self.shared.transmitting.store(transmitting, Ordering::Relaxed);
	}

	pub fn transmitting(&self) -> bool {
		self.shared.transmitting.load(Ordering::Relaxed)
	}

	pub fn set_noise_suppression(&self, enabled: bool) {
		self.shared.noise_suppression.store(enabled, Ordering::Relaxed);
	}

	pub fn input_level(&self) -> f32 {
		self.shared.input_level.load()
	}

	pub fn input_level_db(&self) -> f32 {
		self.shared.input_db.load()
	}

	pub fn output_level(&self) -> f32 {
		self.shared.output_level.load()
	}

	pub fn stats(&self) -> VoiceStats {
		self.shared.counters.snapshot()
	}

	pub fn dave(&self) -> Arc<DaveStats> {
		self.dave.clone()
	}

	pub fn set_video_sink(&self, sink: Option<Arc<dyn VideoSink>>) {
		*self.shared.video.sink.lock() = sink;
	}

	pub fn set_video_wants(&self, wants: Vec<VideoWant>) {
		*self.shared.video.wants.lock() = wants;
		let _ = self.control.send(Control::VideoWants);
	}

	pub fn video_users(&self) -> Vec<Snowflake> {
		let mut users: Vec<Snowflake> = self.shared.video.remote.lock().keys().copied().collect();
		users.sort_unstable();
		users
	}

	pub fn video_stats(&self) -> VideoStats {
		self.shared.video.stats()
	}

	pub fn feed_input(&self, pcm: &[i16]) {
		let frame = pcm
			.iter()
			.map(|sample| f32::from(*sample) / f32::from(i16::MAX))
			.collect();
		self.shared.manual_input.lock().push_back(frame);
	}

	pub fn take_output(&self) -> Vec<MixedFrame> {
		self.shared.take_output()
	}

	pub fn disconnect(&self) {
		let _ = self.shutdown.send(true);
	}
}

impl Drop for VoiceConnection {
	fn drop(&mut self) {
		let _ = self.shutdown.send(true);
	}
}
