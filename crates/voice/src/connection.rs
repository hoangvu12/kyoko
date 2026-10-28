use std::sync::Arc;
use std::sync::atomic::Ordering;

use dave::{DaveSession, DaveStats};
use net::Trust;
use parking_lot::Mutex;
use protocol::{Snowflake, Token};
use tokio::sync::{mpsc, watch};

use crate::audio::{AudioEngine, Rings};
use crate::error::{Result, VoiceError};
use crate::event::VoiceEvent;
use crate::media::SharedTransform;
use crate::shared::{InputMode, MixedFrame, Shared, VoiceStats};
use crate::signal::{Browser, Credentials, establish};
use crate::supervisor::{Signals, Supervisor};
use crate::transform::DaveTransform;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioIo {
	Devices {
		input: Option<String>,
		output: Option<String>,
	},
	Manual,
}

pub struct VoiceOptions {
	pub audio: AudioIo,
	pub trust: Trust,
	pub user_agent: String,
	pub locale: String,
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
		}
	}
}

pub struct VoiceConnection {
	shared: Arc<Shared>,
	dave: Arc<DaveStats>,
	shutdown: watch::Sender<bool>,
}

impl VoiceConnection {
	pub async fn connect(params: VoiceParams, options: VoiceOptions) -> Result<Self> {
		let browser = Browser {
			user_agent: &options.user_agent,
			locale: &options.locale,
			trust: &options.trust,
		};
		let credentials = Credentials::new(
			[params.server_id(), params.channel_id, params.user_id],
			params.session_id,
			params.token,
			&params.endpoint,
			&browser,
		)?;
		let established = establish(&credentials).await?;
		let dave = DaveSession::new(params.user_id.get(), params.channel_id.get())?;
		let dave_stats = dave.stats();
		let rings = Arc::new(Rings::new());
		let shared = Arc::new(Shared::new(rings.clone()));
		let audio_io = match options.audio {
			AudioIo::Devices { input, output } => {
				let engine = tokio::task::spawn_blocking(move || AudioEngine::start(rings, input, output))
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
				control,
				commands,
			},
		)?;
		tokio::spawn(supervisor.run());
		Ok(Self {
			shared,
			dave: dave_stats,
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
		if let Some(engine) = self.shared.engine.lock().as_ref() {
			engine.set_input(id);
		}
	}

	pub fn set_output_device(&self, id: Option<String>) {
		if let Some(engine) = self.shared.engine.lock().as_ref() {
			engine.set_output(id);
		}
	}

	pub fn set_user_volume(&self, user_id: Snowflake, volume: f32) {
		self.shared
			.user_volumes
			.lock()
			.insert(user_id, volume.clamp(0.0, MAX_VOLUME));
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

	pub fn set_noise_suppression(&self, enabled: bool) {
		self.shared.noise_suppression.store(enabled, Ordering::Relaxed);
	}

	pub fn input_level(&self) -> f32 {
		self.shared.input_level.load()
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
