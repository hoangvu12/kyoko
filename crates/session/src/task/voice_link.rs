use std::sync::Arc;

use gateway::{SendError, VoiceStateRequest};
use protocol::{ChannelType, Snowflake, Token, VoiceServerUpdate, VoiceState};
use state::{LocalEvent, VoiceStatus};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use voice::{AudioIo, StopReason, VoiceConnection, VoiceError, VoiceEvent, VoiceOptions, VoiceParams};

use super::{Core, Internal};

const ALLOW_VOICE_RECORDING: u64 = 1 << 1;
const JOIN_FAILED: &str = "Couldn't connect to voice. The channel may be full.";
const NOT_A_VOICE_CHANNEL: &str = "That isn't a voice channel.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Target {
	guild_id: Snowflake,
	channel_id: Snowflake,
}

struct Server {
	token: Token,
	endpoint: String,
}

#[derive(Default)]
pub(crate) struct VoiceLink {
	target: Option<Target>,
	session_id: Option<String>,
	server: Option<Server>,
	connection: Option<Arc<VoiceConnection>>,
	pump: Option<JoinHandle<()>>,
	connecting: bool,
	generation: u64,
	self_mute: bool,
	self_deaf: bool,
	server_mute: bool,
	server_deaf: bool,
	regions: Vec<String>,
}

impl VoiceLink {
	fn sends_muted(&self) -> bool {
		self.self_mute || self.self_deaf
	}

	fn input_off(&self) -> bool {
		self.sends_muted() || self.server_mute
	}

	fn output_off(&self) -> bool {
		self.self_deaf || self.server_deaf
	}
}

fn stop_reason(reason: StopReason) -> Option<String> {
	match reason {
		StopReason::Left => None,
		StopReason::Disconnected => Some("You were disconnected from the voice channel.".to_owned()),
		StopReason::E2eeRequired => Some("This call needs end-to-end encryption.".to_owned()),
		StopReason::CallEnded => Some("The call ended.".to_owned()),
		StopReason::RateLimited => Some("Voice is rate limited. Try again in a moment.".to_owned()),
		StopReason::AuthenticationFailed | StopReason::ServerNotFound | StopReason::Unreachable => {
			Some("The voice connection was lost.".to_owned())
		}
		StopReason::Closed(code) => Some(format!("The voice connection closed ({code}).")),
	}
}

impl Core {
	pub(super) fn join_voice(&mut self, guild_id: Snowflake, channel_id: Snowflake) {
		let is_voice = self.state.read().guild(guild_id).is_some_and(|guild| {
			guild.channels.get(&channel_id).is_some_and(|channel| {
				matches!(
					channel.kind,
					ChannelType::GuildVoice | ChannelType::GuildStageVoice
				)
			})
		});
		if !is_voice {
			self.reject(NOT_A_VOICE_CHANNEL);
			return;
		}
		let target = Target { guild_id, channel_id };
		if self.voice.target == Some(target) {
			return;
		}
		self.drop_connection();
		self.voice.target = Some(target);
		self.voice.session_id = None;
		self.voice.server = None;
		self.voice.server_mute = false;
		self.voice.server_deaf = false;
		self.announce(VoiceStatus::Connecting);
		self.request_voice_state();
	}

	pub(super) fn leave_voice(&mut self) {
		self.end_voice(None, true);
	}

	pub(super) fn set_self_mute(&mut self, muted: bool) {
		if !muted && self.voice.self_deaf {
			self.voice.self_deaf = false;
		}
		self.voice.self_mute = muted;
		self.publish_self_state();
	}

	pub(super) fn set_self_deaf(&mut self, deafened: bool) {
		self.voice.self_deaf = deafened;
		self.publish_self_state();
	}

	pub(super) fn voice_ready(&mut self, regions: Vec<String>) {
		self.voice.regions = regions;
		if self.voice.target.is_some() {
			self.drop_connection();
			self.voice.session_id = None;
			self.voice.server = None;
			self.announce(VoiceStatus::Connecting);
			self.request_voice_state();
		}
	}

	pub(super) fn voice_state_update(&mut self, update: &VoiceState) {
		let Some(target) = self.voice.target else {
			return;
		};
		if update.guild_id != Some(target.guild_id)
			|| self.state.read().current_user_id() != Some(update.user_id)
		{
			return;
		}
		match update.channel_id {
			Some(channel_id) => {
				if channel_id != target.channel_id {
					self.voice.target = Some(Target { channel_id, ..target });
					let status = self
						.state
						.read()
						.voice()
						.map_or(VoiceStatus::Connecting, |session| session.status.clone());
					self.announce(status);
				}
				if let Some(session_id) = &update.session_id {
					self.voice.session_id = Some(session_id.clone());
				}
				self.voice.server_mute = update.mute;
				self.voice.server_deaf = update.deaf;
				self.apply_to_connection();
				self.try_connect();
			}
			None if self.voice.session_id.is_some() => {
				self.end_voice(stop_reason(StopReason::Disconnected), false);
			}
			None => {}
		}
	}

	pub(super) fn voice_server_update(&mut self, update: VoiceServerUpdate) {
		let Some(target) = self.voice.target else {
			return;
		};
		let (true, Some(token), Some(endpoint)) = (
			update.guild_id == Some(target.guild_id),
			update.token,
			update.endpoint,
		) else {
			return;
		};
		if self.voice.connection.is_some() || self.voice.connecting {
			self.drop_connection();
			self.announce(VoiceStatus::Connecting);
		}
		self.voice.server = Some(Server { token, endpoint });
		self.try_connect();
	}

	pub(super) fn on_voice_join_timeout(&mut self, generation: u64) {
		if generation == self.voice.generation && self.voice.connection.is_none() && !self.voice.connecting {
			self.end_voice(None, true);
			self.reject(JOIN_FAILED);
		}
	}

	pub(super) fn on_voice_connected(
		&mut self,
		generation: u64,
		result: Result<Arc<VoiceConnection>, VoiceError>,
	) {
		if generation != self.voice.generation {
			if let Ok(connection) = result {
				connection.disconnect();
			}
			return;
		}
		self.voice.connecting = false;
		match result {
			Ok(connection) => self.adopt_connection(connection),
			Err(error) => {
				tracing::warn!(%error, "voice connection failed");
				self.end_voice(None, true);
				self.reject(JOIN_FAILED);
			}
		}
	}

	pub(super) fn on_voice_event(&mut self, generation: u64, event: VoiceEvent) {
		if generation != self.voice.generation {
			return;
		}
		match event {
			VoiceEvent::Connected { .. } | VoiceEvent::Resumed => self.announce(VoiceStatus::Connected),
			VoiceEvent::Reconnecting { .. } => self.announce(VoiceStatus::Reconnecting),
			VoiceEvent::Speaking { user_id, speaking } => {
				self.apply_local(LocalEvent::Speaking { user_id, speaking });
			}
			VoiceEvent::UserLeft { user_id } => {
				self.apply_local(LocalEvent::Speaking {
					user_id,
					speaking: false,
				});
			}
			VoiceEvent::Stopped(reason) => {
				let notify = reason != StopReason::Disconnected;
				self.end_voice(stop_reason(reason), notify);
			}
			VoiceEvent::Dave {
				epoch,
				version,
				members,
			} => {
				tracing::debug!(epoch, version, members = members.len(), "voice e2ee epoch");
			}
			VoiceEvent::DaveFailure(reason) => tracing::warn!(%reason, "voice e2ee failure"),
			VoiceEvent::UserJoined { .. } | VoiceEvent::Stats { .. } => {}
		}
	}

	pub(super) fn close_voice(&mut self) {
		self.drop_connection();
	}

	fn request_voice_state(&mut self) {
		let Some(target) = self.voice.target else {
			return;
		};
		if let Err(error) = self.send_voice_state(target.guild_id, Some(target.channel_id)) {
			tracing::warn!(%error, "voice state was not sent");
			self.end_voice(None, false);
			self.reject(JOIN_FAILED);
			return;
		}
		let generation = self.voice.generation;
		let timeout = self.voice_shared.setup.lock().join_timeout;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			sleep(timeout).await;
			let _ = internal.send(Internal::VoiceJoinTimeout(generation));
		});
	}

	fn publish_self_state(&mut self) {
		self.apply_local(LocalEvent::VoiceSelf {
			self_mute: self.voice.sends_muted(),
			self_deaf: self.voice.self_deaf,
		});
		self.apply_to_connection();
		if let Some(target) = self.voice.target
			&& let Err(error) = self.send_voice_state(target.guild_id, Some(target.channel_id))
		{
			tracing::warn!(%error, "voice state was not sent");
		}
	}

	fn apply_to_connection(&self) {
		if let Some(connection) = &self.voice.connection {
			self.apply_to_connection_of(connection);
		}
	}

	fn send_voice_state(&self, guild_id: Snowflake, channel_id: Option<Snowflake>) -> Result<(), SendError> {
		self.gateway.update_voice_state(&VoiceStateRequest {
			guild_id: Some(guild_id),
			channel_id,
			self_mute: self.voice.sends_muted(),
			self_deaf: self.voice.self_deaf,
			self_video: false,
			preferred_region: self.voice.regions.first().map(String::as_str),
			preferred_regions: &self.voice.regions,
			flags: ALLOW_VOICE_RECORDING,
		})
	}

	fn announce(&mut self, status: VoiceStatus) {
		let Some(target) = self.voice.target else {
			return;
		};
		self.apply_local(LocalEvent::VoiceConnection {
			status,
			channel_id: target.channel_id,
			guild_id: Some(target.guild_id),
		});
	}

	fn end_voice(&mut self, reason: Option<String>, notify_gateway: bool) {
		let Some(target) = self.voice.target.take() else {
			return;
		};
		self.drop_connection();
		self.voice.session_id = None;
		self.voice.server = None;
		self.voice.server_mute = false;
		self.voice.server_deaf = false;
		if notify_gateway && let Err(error) = self.send_voice_state(target.guild_id, None) {
			tracing::warn!(%error, "voice state was not sent");
		}
		self.apply_local(LocalEvent::VoiceConnection {
			status: VoiceStatus::Disconnected { reason: None },
			channel_id: target.channel_id,
			guild_id: Some(target.guild_id),
		});
		if let Some(reason) = reason {
			self.reject(reason);
		}
	}

	fn drop_connection(&mut self) {
		self.voice.generation += 1;
		self.voice.connecting = false;
		if let Some(pump) = self.voice.pump.take() {
			pump.abort();
		}
		if let Some(connection) = self.voice.connection.take() {
			connection.disconnect();
		}
		*self.voice_shared.connection.lock() = None;
	}

	fn try_connect(&mut self) {
		if self.voice.connecting || self.voice.connection.is_some() {
			return;
		}
		let (Some(target), Some(session_id), Some(server), Some(user_id)) = (
			self.voice.target,
			self.voice.session_id.clone(),
			self.voice.server.as_ref(),
			self.state.read().current_user_id(),
		) else {
			return;
		};
		let params = VoiceParams {
			guild_id: Some(target.guild_id),
			channel_id: target.channel_id,
			user_id,
			session_id,
			token: Token::new(server.token.as_str()),
			endpoint: server.endpoint.clone(),
		};
		let options = self.voice_options();
		self.voice.connecting = true;
		let generation = self.voice.generation;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let result = VoiceConnection::connect(params, options).await;
			let _ = internal.send(Internal::VoiceConnected {
				generation,
				result: result.map(Arc::new),
			});
		});
	}

	fn voice_options(&self) -> VoiceOptions {
		let setup = self.voice_shared.setup.lock().clone();
		let audio = match setup.audio {
			AudioIo::Devices { .. } => {
				let settings = self.store.settings.read();
				AudioIo::Devices {
					input: settings.voice.input_device.clone(),
					output: settings.voice.output_device.clone(),
				}
			}
			AudioIo::Manual => AudioIo::Manual,
		};
		VoiceOptions {
			audio,
			trust: setup.trust,
			user_agent: self.rest.identity().fingerprint().user_agent().to_owned(),
			locale: self.locale.clone(),
		}
	}

	fn adopt_connection(&mut self, connection: Arc<VoiceConnection>) {
		self.apply_to_connection_of(&connection);
		*self.voice_shared.connection.lock() = Some(connection.clone());
		self.voice.pump = Some(self.spawn_pump(&connection));
		self.voice.connection = Some(connection);
	}

	fn apply_to_connection_of(&self, connection: &VoiceConnection) {
		connection.set_mute(self.voice.input_off());
		connection.set_deaf(self.voice.output_off());
	}

	fn spawn_pump(&self, connection: &VoiceConnection) -> JoinHandle<()> {
		let generation = self.voice.generation;
		let internal = self.internal.clone();
		let mut events = connection.events();
		self.runtime.spawn(async move {
			while let Some(event) = events.recv().await {
				if internal.send(Internal::VoiceEvent { generation, event }).is_err() {
					break;
				}
			}
		})
	}
}
