use std::sync::Arc;
use std::time::Duration;

use protocol::{Snowflake, StreamDelete, StreamServerUpdate, StreamState, Token};
use state::LocalEvent;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use voice::{AudioIo, ConnectionKind, StopReason, VoiceConnection, VoiceError, VoiceEvent, VoiceParams};

use super::{Core, Internal};
use crate::video_feed::VideoView;

const STREAM_TIMEOUT: Duration = Duration::from_secs(15);
const STREAM_UNREACHABLE: &str = "Couldn't connect to the stream.";
const STREAM_LOST: &str = "The stream connection was lost.";
const NOT_IN_THE_CALL: &str = "Join the voice channel to watch this stream.";

pub(crate) fn delete_text(reason: Option<&str>) -> &'static str {
	match reason {
		Some("stream_full") => "This stream is full.",
		Some("user_requested" | "stream_ended") => "The stream has ended.",
		Some("unauthorized") => "You don't have permission to watch this stream.",
		Some("safety_guild_rate_limited") => "Streams in this server are rate limited. Try again later.",
		_ => "The stream is no longer available.",
	}
}

pub(crate) fn stream_owner(key: &str) -> Option<(Snowflake, Snowflake)> {
	let mut parts = key.split(':');
	let kind = parts.next()?;
	let ids: Vec<Snowflake> = parts
		.map(|part| part.parse::<u64>().ok().map(Snowflake))
		.collect::<Option<_>>()?;
	match (kind, ids.as_slice()) {
		("guild", [_, channel_id, owner_id]) | ("call", [channel_id, owner_id]) => {
			Some((*channel_id, *owner_id))
		}
		_ => None,
	}
}

struct Server {
	token: Token,
	endpoint: String,
}

struct Watch {
	key: String,
	owner: Snowflake,
	generation: u64,
	rtc_server_id: Option<Snowflake>,
	rtc_channel_id: Option<Snowflake>,
	server: Option<Server>,
	connection: Option<Arc<VoiceConnection>>,
	connecting: bool,
	pump: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub(crate) struct StreamLink {
	generation: u64,
	watch: Option<Watch>,
}

impl StreamLink {
	fn watching(&mut self, generation: u64) -> Option<&mut Watch> {
		self.watch.as_mut().filter(|watch| watch.generation == generation)
	}

	fn watching_key(&mut self, key: &str) -> Option<&mut Watch> {
		self.watch.as_mut().filter(|watch| watch.key == key)
	}
}

impl Core {
	pub(super) fn watch_stream(&mut self, key: String) {
		let Some((channel_id, owner)) = stream_owner(&key) else {
			return;
		};
		let in_call = self
			.state
			.read()
			.voice()
			.is_some_and(|voice| voice.channel_id == channel_id);
		if !in_call {
			self.reject(NOT_IN_THE_CALL);
			return;
		}
		if self.streams.watch.as_ref().is_some_and(|watch| watch.key == key) {
			return;
		}
		self.end_watch(true, None);
		if let Err(error) = self.gateway.stream_watch(&key) {
			tracing::warn!(%error, "stream watch was not sent");
			self.reject(STREAM_UNREACHABLE);
			return;
		}
		self.streams.generation += 1;
		let generation = self.streams.generation;
		self.streams.watch = Some(Watch {
			key: key.clone(),
			owner,
			generation,
			rtc_server_id: None,
			rtc_channel_id: None,
			server: None,
			connection: None,
			connecting: false,
			pump: None,
		});
		self.apply_local(LocalEvent::Watching(Some(key)));
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			sleep(STREAM_TIMEOUT).await;
			let _ = internal.send(Internal::StreamTimeout(generation));
		});
	}

	pub(super) fn stop_watching(&mut self, key: &str) {
		if self.streams.watch.as_ref().is_some_and(|watch| watch.key == key) {
			self.end_watch(true, None);
		}
	}

	pub(super) fn end_streams(&mut self, notify_gateway: bool) {
		self.end_watch(notify_gateway, None);
	}

	pub(super) fn set_stream_views(&mut self, views: Vec<VideoView>) {
		self.voice_shared.stream_feed.set_views(&views);
		*self.voice_shared.stream_views.lock() = views;
		if let Some(connection) = self
			.streams
			.watch
			.as_ref()
			.and_then(|watch| watch.connection.clone())
		{
			self.apply_stream_wants(&connection);
		}
	}

	pub(super) fn stream_created(&mut self, stream: &StreamState) {
		let Some(watch) = self.streams.watching_key(&stream.stream_key) else {
			return;
		};
		watch.rtc_server_id = stream.rtc_server_id.or(watch.rtc_server_id);
		watch.rtc_channel_id = stream.rtc_channel_id.or(watch.rtc_channel_id);
		let generation = watch.generation;
		self.try_connect_stream(generation);
	}

	pub(super) fn stream_server_updated(&mut self, update: StreamServerUpdate) {
		let Some(watch) = self.streams.watching_key(&update.stream_key) else {
			return;
		};
		let generation = watch.generation;
		Self::drop_stream_connection(watch);
		*self.voice_shared.stream_connection.lock() = None;
		watch.server = match (update.token, update.endpoint) {
			(Some(token), Some(endpoint)) => Some(Server { token, endpoint }),
			_ => None,
		};
		self.voice_shared.stream_feed.clear();
		self.try_connect_stream(generation);
	}

	pub(super) fn stream_deleted(&mut self, delete: &StreamDelete) {
		if self
			.streams
			.watch
			.as_ref()
			.is_some_and(|watch| watch.key == delete.stream_key)
		{
			self.end_watch(false, Some(delete_text(delete.reason.as_deref())));
		}
	}

	pub(super) fn on_stream_timeout(&mut self, generation: u64) {
		let waiting = self
			.streams
			.watching(generation)
			.is_some_and(|watch| watch.connection.is_none() && !watch.connecting);
		if waiting {
			self.end_watch(true, Some(STREAM_UNREACHABLE));
		}
	}

	pub(super) fn on_stream_connected(
		&mut self,
		generation: u64,
		result: Result<Arc<VoiceConnection>, VoiceError>,
	) {
		let Some(watch) = self.streams.watching(generation) else {
			if let Ok(connection) = result {
				connection.disconnect();
			}
			return;
		};
		watch.connecting = false;
		let connection = match result {
			Ok(connection) => connection,
			Err(error) => {
				tracing::warn!(%error, "stream connection failed");
				self.end_watch(true, Some(STREAM_UNREACHABLE));
				return;
			}
		};
		connection.set_mute(true);
		connection.set_deaf(true);
		connection.set_video_sink(Some(self.voice_shared.stream_feed.sink()));
		let internal = self.internal.clone();
		let mut events = connection.events();
		watch.pump = Some(self.runtime.spawn(async move {
			while let Some(event) = events.recv().await {
				if internal
					.send(Internal::StreamEvent { generation, event })
					.is_err()
				{
					break;
				}
			}
		}));
		watch.connection = Some(connection.clone());
		*self.voice_shared.stream_connection.lock() = Some(connection.clone());
		self.apply_stream_wants(&connection);
	}

	pub(super) fn on_stream_event(&mut self, generation: u64, event: VoiceEvent) {
		if self.streams.watching(generation).is_none() {
			return;
		}
		match event {
			VoiceEvent::Stopped(StopReason::Left) => {}
			VoiceEvent::Stopped(reason) => {
				tracing::debug!(?reason, "stream connection stopped");
				self.end_watch(true, Some(STREAM_LOST));
			}
			VoiceEvent::Dave {
				epoch,
				version,
				members,
			} => {
				tracing::debug!(epoch, version, members = members.len(), "stream e2ee epoch");
			}
			VoiceEvent::DaveFailure(reason) => tracing::warn!(%reason, "stream e2ee failure"),
			_ => {}
		}
	}

	fn apply_stream_wants(&self, connection: &VoiceConnection) {
		let wants = self
			.voice_shared
			.stream_views
			.lock()
			.iter()
			.map(VideoView::want)
			.collect();
		connection.set_video_wants(wants);
	}

	fn try_connect_stream(&mut self, generation: u64) {
		let user_id = self.state.read().current_user_id();
		let session_id = self.voice_session_id();
		let mut options = self.voice_options();
		options.audio = AudioIo::Manual;
		options.kind = ConnectionKind::Stream;
		let Some(watch) = self.streams.watching(generation) else {
			return;
		};
		if watch.connecting || watch.connection.is_some() {
			return;
		}
		let (Some(user_id), Some(session_id), Some(server_id), Some(channel_id), Some(server)) = (
			user_id,
			session_id,
			watch.rtc_server_id,
			watch.rtc_channel_id,
			watch.server.as_ref(),
		) else {
			return;
		};
		let params = VoiceParams {
			guild_id: Some(server_id),
			channel_id,
			user_id,
			session_id,
			token: Token::new(server.token.as_str()),
			endpoint: server.endpoint.clone(),
		};
		watch.connecting = true;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let result = VoiceConnection::connect(params, options).await;
			let _ = internal.send(Internal::StreamConnected {
				generation,
				result: result.map(Arc::new),
			});
		});
	}

	fn forget_stream_connection(&self, watch: &mut Watch) {
		Self::drop_stream_connection(watch);
		*self.voice_shared.stream_connection.lock() = None;
	}

	fn drop_stream_connection(watch: &mut Watch) {
		watch.connecting = false;
		if let Some(pump) = watch.pump.take() {
			pump.abort();
		}
		if let Some(connection) = watch.connection.take() {
			connection.set_video_sink(None);
			connection.disconnect();
		}
	}

	fn end_watch(&mut self, notify_gateway: bool, reason: Option<&str>) {
		let Some(mut watch) = self.streams.watch.take() else {
			return;
		};
		self.forget_stream_connection(&mut watch);
		self.voice_shared.stream_feed.clear();
		if notify_gateway && let Err(error) = self.gateway.stream_delete(&watch.key) {
			tracing::warn!(%error, "stream delete was not sent");
		}
		tracing::debug!(owner = %watch.owner, "stopped watching a stream");
		self.apply_local(LocalEvent::Watching(None));
		if let Some(reason) = reason {
			self.reject(reason);
		}
	}
}
