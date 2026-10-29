use std::sync::Arc;
use std::time::Duration;

use protocol::{Snowflake, StreamDelete, StreamServerUpdate, StreamState, Token};
use state::LocalEvent;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use voice::video::{ScreenKind, ScreenTarget, VideoError, VideoQuality, VideoSource, fit, open_screen};
use voice::{
	AudioIo, ConnectionKind, SoundSource, SoundTarget, StopReason, VoiceConnection, VoiceError, VoiceEvent,
	VoiceParams, open_loopback,
};

use super::streams::{delete_text, stream_owner};
use super::{Core, Internal};
use crate::voice_setup::{CameraIo, VoiceSetup};

const GO_LIVE_TIMEOUT: Duration = Duration::from_secs(15);
const NOT_IN_A_CALL: &str = "Join a voice channel to go live.";
const GO_LIVE_FAILED: &str = "Couldn't start your stream.";
const STREAM_LOST: &str = "Your stream was disconnected.";
const SOURCE_CLOSED: &str = "The window you picked was closed.";
const CAPTURE_STOPPED: &str = "Your screen capture stopped.";
const VIDEO_UNAVAILABLE: &str = "Video unavailable. Windows has no H.264 video encoder here.";

pub(crate) struct Share {
	generation: u64,
	source: ScreenTarget,
	quality: VideoQuality,
	sound: bool,
	capture: u64,
	key: Option<String>,
	rtc_server_id: Option<Snowflake>,
	rtc_channel_id: Option<Snowflake>,
	server: Option<(Token, String)>,
	connection: Option<Arc<VoiceConnection>>,
	connecting: bool,
	pump: Option<JoinHandle<()>>,
	paused: bool,
	hidden: bool,
}

impl Share {
	fn owns(&self, key: &str) -> bool {
		self.key.as_deref().is_none_or(|own| own == key)
	}
}

fn capture_size(source: &ScreenTarget, quality: VideoQuality) -> VideoQuality {
	let (width, height) = fit(source.width, source.height, quality.width, quality.height);
	VideoQuality {
		width,
		height,
		..quality
	}
}

async fn open_source(
	setup: &VoiceSetup,
	source: ScreenTarget,
	quality: VideoQuality,
) -> Result<Box<dyn VideoSource>, VideoError> {
	match setup.camera {
		CameraIo::Pattern => Ok(setup.capture.screen(capture_size(&source, quality))),
		CameraIo::Devices => tokio::task::spawn_blocking(move || open_screen(&source, quality))
			.await
			.unwrap_or(Err(VideoError::Stopped)),
	}
}

fn sound_target(source: &ScreenTarget) -> Option<SoundTarget> {
	let own = std::process::id();
	match source.kind {
		ScreenKind::Screen => Some(SoundTarget::Everything { except: own }),
		ScreenKind::Window if source.process == 0 || source.process == own => None,
		ScreenKind::Window => Some(SoundTarget::App {
			process: source.process,
		}),
	}
}

async fn open_sound(setup: &VoiceSetup, target: SoundTarget) -> Result<Box<dyn SoundSource>, VoiceError> {
	match setup.camera {
		CameraIo::Pattern => Ok(setup.capture.sound(target)),
		CameraIo::Devices => tokio::task::spawn_blocking(move || open_loopback(target))
			.await
			.map_err(|error| VoiceError::Audio(error.to_string()))?,
	}
}

async fn share_sound(setup: &VoiceSetup, source: &ScreenTarget, connection: &VoiceConnection) {
	let Some(target) = sound_target(source) else {
		return;
	};
	let started = match open_sound(setup, target).await {
		Ok(sound) => connection.start_sound(sound),
		Err(error) => Err(error),
	};
	match started {
		Ok(()) => tracing::debug!("sharing the stream's sound"),
		Err(error) => tracing::warn!(%error, "the stream's sound was not shared"),
	}
}

impl Core {
	pub(super) fn go_live(&mut self, source: ScreenTarget, quality: VideoQuality, sound: bool) {
		let call = self
			.state
			.read()
			.voice()
			.map(|voice| (voice.guild_id, voice.channel_id));
		let Some((guild_id, channel_id)) = call else {
			self.reject(NOT_IN_A_CALL);
			return;
		};
		if self.streams.share.is_some() {
			self.swap_source(source, quality, sound);
			return;
		}
		let region = self.preferred_region();
		if let Err(error) = self
			.gateway
			.stream_create(guild_id, channel_id, region.as_deref())
		{
			tracing::warn!(%error, "stream create was not sent");
			self.reject(GO_LIVE_FAILED);
			return;
		}
		let generation = self.streams.next_generation();
		self.streams.share = Some(Share {
			generation,
			source,
			quality,
			sound,
			capture: 0,
			key: None,
			rtc_server_id: None,
			rtc_channel_id: None,
			server: None,
			connection: None,
			connecting: false,
			pump: None,
			paused: false,
			hidden: false,
		});
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			sleep(GO_LIVE_TIMEOUT).await;
			let _ = internal.send(Internal::StreamTimeout(generation));
		});
	}

	fn swap_source(&mut self, source: ScreenTarget, quality: VideoQuality, sound: bool) {
		let Some(share) = self.streams.share.as_mut() else {
			return;
		};
		if share.source == source && share.quality == quality && share.sound == sound {
			return;
		}
		share.source = source;
		share.quality = quality;
		share.sound = sound;
		share.capture += 1;
		let connection = share.connection.clone();
		if std::mem::take(&mut share.hidden) {
			self.set_stream_paused(false);
		}
		if let Some(connection) = connection {
			tracing::debug!("swapping the stream's source");
			self.start_capture(connection, true);
		}
	}

	fn start_capture(&self, connection: Arc<VoiceConnection>, swap: bool) {
		let Some(share) = self.streams.share.as_ref() else {
			return;
		};
		let setup = self.voice_shared.setup.lock().clone();
		let (source, quality, sound) = (share.source.clone(), share.quality, share.sound);
		let (generation, capture) = (share.generation, share.capture);
		connection.stop_sound();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let result = match open_source(&setup, source.clone(), quality).await {
				Ok(video) if swap => connection.replace_screen(video, quality, setup.codecs).await,
				Ok(video) => connection.start_screen(video, quality, setup.codecs).await,
				Err(error) => Err(error),
			};
			if result.is_ok() && sound {
				share_sound(&setup, &source, &connection).await;
			}
			let _ = internal.send(Internal::ShareStarted {
				generation,
				capture,
				result,
			});
		});
	}

	pub(super) fn stop_go_live(&mut self) {
		self.end_share(true, None);
	}

	pub(super) fn set_stream_paused(&mut self, paused: bool) {
		let Some(share) = self.streams.share.as_mut() else {
			return;
		};
		if share.paused == paused {
			return;
		}
		share.paused = paused;
		if let Some(connection) = &share.connection {
			connection.set_video_paused(paused);
		}
		if let Some(key) = share.key.clone() {
			self.send_paused(&key, paused);
		}
	}

	fn send_paused(&self, key: &str, paused: bool) {
		if let Err(error) = self.gateway.stream_set_paused(key, paused) {
			tracing::warn!(%error, "stream pause was not sent");
		}
	}

	pub(super) fn is_share(&self, generation: u64) -> bool {
		self.streams
			.share
			.as_ref()
			.is_some_and(|share| share.generation == generation)
	}

	fn own_stream(&self, key: &str) -> bool {
		let me = self.state.read().current_user_id();
		stream_owner(key).is_some_and(|(_, owner)| Some(owner) == me)
			&& self.streams.share.as_ref().is_some_and(|share| share.owns(key))
	}

	pub(super) fn share_created(&mut self, stream: &StreamState) -> bool {
		if !self.own_stream(&stream.stream_key) {
			return false;
		}
		let Some(share) = self.streams.share.as_mut() else {
			return false;
		};
		let announce_pause = share.key.is_none() && share.paused;
		share.key = Some(stream.stream_key.clone());
		share.rtc_server_id = stream.rtc_server_id.or(share.rtc_server_id);
		share.rtc_channel_id = stream.rtc_channel_id.or(share.rtc_channel_id);
		if announce_pause {
			self.send_paused(&stream.stream_key, true);
		}
		self.try_connect_share();
		true
	}

	pub(super) fn share_server_updated(&mut self, update: StreamServerUpdate) -> Option<StreamServerUpdate> {
		if !self.own_stream(&update.stream_key) {
			return Some(update);
		}
		let share = self.streams.share.as_mut()?;
		share.key = Some(update.stream_key.clone());
		Self::drop_share_connection(share);
		*self.voice_shared.share_connection.lock() = None;
		share.server = update.token.zip(update.endpoint);
		self.try_connect_share();
		None
	}

	pub(super) fn share_deleted(&mut self, delete: &StreamDelete) -> bool {
		let ours = self
			.streams
			.share
			.as_ref()
			.is_some_and(|share| share.key.as_deref() == Some(delete.stream_key.as_str()));
		if ours {
			self.end_share(false, Some(delete_text(delete.reason.as_deref())));
		}
		ours
	}

	pub(super) fn on_share_timeout(&mut self) {
		let waiting = self
			.streams
			.share
			.as_ref()
			.is_some_and(|share| share.connection.is_none() && !share.connecting);
		if waiting {
			self.end_share(true, Some(GO_LIVE_FAILED));
		}
	}

	fn try_connect_share(&mut self) {
		let user_id = self.state.read().current_user_id();
		let session_id = self.voice_session_id();
		let mut options = self.voice_options();
		options.audio = AudioIo::Manual;
		options.kind = ConnectionKind::Stream;
		let Some(share) = self.streams.share.as_mut() else {
			return;
		};
		if share.connecting || share.connection.is_some() {
			return;
		}
		let (Some(user_id), Some(session_id), Some(server_id), Some(channel_id), Some((token, endpoint))) = (
			user_id,
			session_id,
			share.rtc_server_id,
			share.rtc_channel_id,
			share.server.as_ref(),
		) else {
			return;
		};
		let params = VoiceParams {
			guild_id: Some(server_id),
			channel_id,
			user_id,
			session_id,
			token: Token::new(token.as_str()),
			endpoint: endpoint.clone(),
		};
		share.connecting = true;
		let generation = share.generation;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let result = VoiceConnection::connect(params, options).await;
			let _ = internal.send(Internal::StreamConnected {
				generation,
				result: result.map(Arc::new),
			});
		});
	}

	pub(super) fn on_share_connected(&mut self, result: Result<Arc<VoiceConnection>, VoiceError>) {
		let Some(share) = self.streams.share.as_mut() else {
			return;
		};
		share.connecting = false;
		let connection = match result {
			Ok(connection) => connection,
			Err(error) => {
				tracing::warn!(%error, "go live connection failed");
				self.end_share(true, Some(GO_LIVE_FAILED));
				return;
			}
		};
		connection.set_mute(true);
		connection.set_deaf(true);
		connection.set_video_paused(share.paused);
		connection.set_video_sink(Some(self.voice_shared.stream_feed.sink()));
		let generation = share.generation;
		let internal = self.internal.clone();
		let mut events = connection.events();
		share.pump = Some(self.runtime.spawn(async move {
			while let Some(event) = events.recv().await {
				if internal
					.send(Internal::StreamEvent { generation, event })
					.is_err()
				{
					break;
				}
			}
		}));
		share.connection = Some(connection.clone());
		*self.voice_shared.share_connection.lock() = Some(connection.clone());
		self.start_capture(connection, false);
	}

	pub(super) fn on_share_started(&mut self, generation: u64, capture: u64, result: Result<(), VideoError>) {
		let current = self
			.streams
			.share
			.as_ref()
			.is_some_and(|share| share.generation == generation && share.capture == capture);
		if !current {
			return;
		}
		match result {
			Ok(()) => tracing::debug!("went live"),
			Err(error) => {
				tracing::warn!(%error, "the stream's video did not start");
				let reason = if error.unavailable() {
					VIDEO_UNAVAILABLE
				} else if matches!(error, VideoError::Ended) {
					SOURCE_CLOSED
				} else {
					GO_LIVE_FAILED
				};
				self.end_share(true, Some(reason));
			}
		}
	}

	pub(super) fn on_share_event(&mut self, event: VoiceEvent) {
		match event {
			VoiceEvent::Stopped(StopReason::Left) => {}
			VoiceEvent::Stopped(reason) => {
				tracing::debug!(?reason, "go live connection stopped");
				self.end_share(true, Some(STREAM_LOST));
			}
			VoiceEvent::VideoSourceEnded => self.end_share(true, Some(SOURCE_CLOSED)),
			VoiceEvent::VideoSourceHidden(hidden) => self.on_source_hidden(hidden),
			VoiceEvent::CameraStopped => self.end_share(true, Some(CAPTURE_STOPPED)),
			VoiceEvent::Dave {
				epoch,
				version,
				members,
			} => {
				tracing::debug!(epoch, version, members = members.len(), "go live e2ee epoch");
			}
			VoiceEvent::DaveFailure(reason) => tracing::warn!(%reason, "go live e2ee failure"),
			_ => {}
		}
	}

	fn on_source_hidden(&mut self, hidden: bool) {
		let Some(share) = self.streams.share.as_mut() else {
			return;
		};
		if share.hidden == hidden {
			return;
		}
		share.hidden = hidden;
		tracing::debug!(hidden, "the shared window was minimised or restored");
		self.set_stream_paused(hidden);
	}

	fn drop_share_connection(share: &mut Share) {
		share.connecting = false;
		if let Some(pump) = share.pump.take() {
			pump.abort();
		}
		if let Some(connection) = share.connection.take() {
			connection.stop_sound();
			connection.stop_camera();
			connection.disconnect();
		}
	}

	pub(super) fn end_share(&mut self, notify_gateway: bool, reason: Option<&str>) {
		let Some(mut share) = self.streams.share.take() else {
			return;
		};
		Self::drop_share_connection(&mut share);
		*self.voice_shared.share_connection.lock() = None;
		if notify_gateway
			&& let Some(key) = &share.key
			&& let Err(error) = self.gateway.stream_delete(key)
		{
			tracing::warn!(%error, "stream delete was not sent");
		}
		tracing::debug!("stopped going live");
		self.apply_local(LocalEvent::ScreenShareChanged(None));
		if let Some(reason) = reason {
			self.reject(reason);
		}
	}
}
