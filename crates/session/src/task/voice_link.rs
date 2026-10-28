use std::sync::Arc;
use std::time::Duration;

use gateway::{SendError, VoiceStateRequest};
use protocol::{ChannelType, Snowflake, Token, VoiceServerUpdate, VoiceState};
use state::{LocalEvent, UserAudio, VoiceStatus};
use store::VoiceSettings;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use voice::{
	AudioIo, ConnectionKind, InputMode, StopReason, VoiceConnection, VoiceError, VoiceEvent, VoiceOptions,
	VoiceParams,
};

use super::{Core, Internal};
use crate::persist::{self, SavedUserAudio};

const ALLOW_VOICE_RECORDING: u64 = 1 << 1;
const JOIN_FAILED: &str = "Couldn't connect to voice. The channel may be full.";
const NOT_A_VOICE_CHANNEL: &str = "That isn't a voice channel.";
const AUTO_SENSITIVITY_DB: f32 = -50.0;
const FULL_VOLUME_PERCENT: f32 = 100.0;

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
	transmitting: bool,
	presses: u64,
	regions: Vec<String>,
}

impl VoiceLink {
	fn sends_muted(&self) -> bool {
		self.self_mute || self.self_deaf
	}

	fn input_off(&self) -> bool {
		self.sends_muted() || self.server_mute
	}

	pub(super) fn output_off(&self) -> bool {
		self.self_deaf || self.server_deaf
	}
}

fn input_mode(settings: &VoiceSettings) -> InputMode {
	if settings.push_to_talk {
		InputMode::PushToTalk
	} else {
		InputMode::VoiceActivity
	}
}

fn sensitivity_db(settings: &VoiceSettings) -> f32 {
	if settings.auto_sensitivity {
		AUTO_SENSITIVITY_DB
	} else {
		f32::from(settings.sensitivity_db)
	}
}

pub(super) fn volume(percent: u8) -> f32 {
	f32::from(percent) / FULL_VOLUME_PERCENT
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
		self.end_streams(true);
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
			self.end_streams(false);
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
			VoiceEvent::UserJoined { user_id } => {
				self.apply_local(LocalEvent::VoicePeer {
					user_id,
					present: true,
				});
			}
			VoiceEvent::UserLeft { user_id } => {
				self.apply_local(LocalEvent::VoicePeer {
					user_id,
					present: false,
				});
			}
			VoiceEvent::VideoStarted { user_id } => self.remote_video(user_id, true),
			VoiceEvent::VideoStopped { user_id } => self.remote_video(user_id, false),
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
			VoiceEvent::Stats { .. } => {}
		}
	}

	pub(super) fn set_user_volume(&mut self, user_id: Snowflake, volume: u8) {
		let audio = UserAudio {
			volume,
			..self.state.read().user_audio(user_id)
		};
		self.change_user_audio(user_id, audio);
	}

	pub(super) fn set_stream_volume(&mut self, user_id: Snowflake, volume: u8) {
		let audio = UserAudio {
			stream_volume: volume,
			..self.state.read().user_audio(user_id)
		};
		self.change_user_audio(user_id, audio);
	}

	pub(super) fn set_user_muted(&mut self, user_id: Snowflake, muted: bool) {
		let audio = UserAudio {
			muted,
			..self.state.read().user_audio(user_id)
		};
		self.change_user_audio(user_id, audio);
	}

	pub(super) fn restore_user_audio(&mut self, saved: Vec<SavedUserAudio>) {
		for entry in saved {
			self.apply_local(LocalEvent::UserAudio {
				user_id: entry.user_id,
				audio: entry.audio,
			});
		}
		self.apply_user_audio();
	}

	pub(super) fn apply_voice_settings(&mut self) {
		let settings = self.store.settings.read().voice.clone();
		if let Some(connection) = &self.voice.connection {
			let in_use = connection.audio_devices();
			if in_use.input != settings.input_device {
				connection.set_input_device(settings.input_device.clone());
			}
			if in_use.output != settings.output_device {
				connection.set_output_device(settings.output_device.clone());
			}
			self.apply_voice_settings_to(connection, &settings);
		}
		self.apply_stream_settings(&settings);
		self.mic_test_follow(&settings);
	}

	pub(super) fn set_push_to_talk(&mut self, held: bool) {
		self.voice.presses += 1;
		let delay = self.store.settings.read().voice.push_to_talk_release_delay;
		if held || delay == 0 || !self.voice.transmitting {
			self.transmit(held);
			return;
		}
		let press = self.voice.presses;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			sleep(Duration::from_millis(u64::from(delay))).await;
			let _ = internal.send(Internal::PushToTalkReleased(press));
		});
	}

	pub(super) fn push_to_talk_released(&mut self, press: u64) {
		if press == self.voice.presses {
			self.transmit(false);
		}
	}

	fn transmit(&mut self, held: bool) {
		self.voice.transmitting = held;
		if let Some(connection) = &self.voice.connection {
			connection.set_transmitting(held);
		}
	}

	pub(super) fn voice_session_id(&self) -> Option<String> {
		self.voice.session_id.clone()
	}

	pub(super) fn voice_connection(&self) -> Option<Arc<VoiceConnection>> {
		self.voice.connection.clone()
	}

	pub(super) fn audio_io(&self) -> AudioIo {
		match self.voice_shared.setup.lock().audio {
			AudioIo::Devices { .. } => {
				let settings = self.store.settings.read();
				AudioIo::Devices {
					input: settings.voice.input_device.clone(),
					output: settings.voice.output_device.clone(),
				}
			}
			AudioIo::Playback { .. } | AudioIo::Manual => AudioIo::Manual,
		}
	}

	pub(super) fn playback_io(&self) -> AudioIo {
		match self.voice_shared.setup.lock().audio {
			AudioIo::Devices { .. } | AudioIo::Playback { .. } => AudioIo::Playback {
				output: self.store.settings.read().voice.output_device.clone(),
			},
			AudioIo::Manual => AudioIo::Manual,
		}
	}

	fn apply_voice_settings_to(&self, connection: &VoiceConnection, settings: &VoiceSettings) {
		connection.set_input_volume(volume(settings.input_volume));
		connection.set_output_volume(volume(settings.output_volume));
		connection.set_input_mode(input_mode(settings));
		connection.set_vad_threshold_db(sensitivity_db(settings));
		connection.set_noise_suppression(settings.noise_suppression);
		connection.set_transmitting(self.voice.transmitting);
	}

	pub(super) fn voice_channel_updated(&self, channel_id: Snowflake) {
		if self
			.voice
			.target
			.is_some_and(|target| target.channel_id == channel_id)
		{
			self.apply_bitrate();
		}
	}

	fn change_user_audio(&mut self, user_id: Snowflake, audio: UserAudio) {
		if self.state.read().current_user_id() == Some(user_id) {
			return;
		}
		self.apply_local(LocalEvent::UserAudio { user_id, audio });
		if let Some(connection) = &self.voice.connection {
			connection.set_user_volume(user_id, self.state.read().user_audio(user_id).gain());
		}
		self.apply_stream_volume();
		self.store_user_audio();
	}

	fn store_user_audio(&self) {
		let saved: Vec<SavedUserAudio> = self
			.state
			.read()
			.user_audios()
			.map(|(user_id, audio)| SavedUserAudio { user_id, audio })
			.collect();
		match serde_json::to_vec(&saved) {
			Ok(json) => self.persist.submit(persist::Job::Recent {
				key: persist::USER_AUDIO_KEY,
				json,
			}),
			Err(error) => tracing::warn!(%error, "user volumes were not saved"),
		}
	}

	fn apply_user_audio(&self) {
		if let Some(connection) = &self.voice.connection {
			self.apply_user_audio_to(connection);
		}
	}

	fn apply_user_audio_to(&self, connection: &VoiceConnection) {
		for (user_id, audio) in self.state.read().user_audios() {
			connection.set_user_volume(user_id, audio.gain());
		}
	}

	fn apply_bitrate(&self) {
		if let Some(connection) = &self.voice.connection {
			self.apply_bitrate_to(connection);
		}
	}

	fn apply_bitrate_to(&self, connection: &VoiceConnection) {
		let Some(target) = self.voice.target else {
			return;
		};
		let bitrate = self
			.state
			.read()
			.guild(target.guild_id)
			.and_then(|guild| guild.channels.get(&target.channel_id))
			.and_then(|channel| channel.bitrate)
			.and_then(|bitrate| u32::try_from(bitrate).ok())
			.filter(|bitrate| *bitrate > 0);
		if let Some(bitrate) = bitrate {
			connection.set_bitrate(bitrate);
		}
	}

	pub(super) fn close_voice(&mut self) {
		self.end_streams(false);
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
		self.apply_stream_deafen();
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
		self.end_streams(notify_gateway);
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
		self.stop_mic_test();
		self.voice.generation += 1;
		self.voice.connecting = false;
		if let Some(pump) = self.voice.pump.take() {
			pump.abort();
		}
		if let Some(connection) = self.voice.connection.take() {
			connection.disconnect();
		}
		*self.voice_shared.connection.lock() = None;
		self.detach_video();
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

	pub(super) fn voice_options(&self) -> VoiceOptions {
		let audio = self.audio_io();
		let trust = self.voice_shared.setup.lock().trust.clone();
		VoiceOptions {
			audio,
			trust,
			user_agent: self.rest.identity().fingerprint().user_agent().to_owned(),
			locale: self.locale.clone(),
			kind: ConnectionKind::Voice,
		}
	}

	fn adopt_connection(&mut self, connection: Arc<VoiceConnection>) {
		let settings = self.store.settings.read().voice.clone();
		self.apply_voice_settings_to(&connection, &settings);
		self.apply_to_connection_of(&connection);
		self.apply_user_audio_to(&connection);
		self.apply_bitrate_to(&connection);
		self.attach_video(&connection);
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
