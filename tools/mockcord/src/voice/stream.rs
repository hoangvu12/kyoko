use std::collections::HashMap;

use serde_json::json;
use tokio::task::JoinHandle;

use super::{Peer, SCRIPTED, ScriptedCamera, Voice, VoiceControl};
use crate::fixtures::{CURRENT_USER_ID, Fixtures};

const FIRST_RTC_SERVER_ID: u64 = 1_278_201_813_102_755_892;
const RTC_ID_STRIDE: u64 = 4;
const FIRST_STREAM_SSRC: u32 = 8_001;
const STREAM_SSRC_STRIDE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockStream {
	pub key: String,
	pub owner: u64,
	pub guild_id: Option<u64>,
	pub channel_id: u64,
	pub rtc_server_id: u64,
	pub rtc_channel_id: u64,
	pub video_ssrc: u32,
	pub full: bool,
}

impl MockStream {
	pub fn dave_group(&self) -> u64 {
		self.rtc_server_id - 1
	}

	pub fn audio_ssrc(&self) -> u32 {
		self.video_ssrc - 1
	}
}

struct Playing {
	stream: MockStream,
	task: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub(super) struct Streams {
	started: u64,
	playing: HashMap<String, Playing>,
}

impl Drop for Streams {
	fn drop(&mut self) {
		for task in self.playing.values().filter_map(|playing| playing.task.as_ref()) {
			task.abort();
		}
	}
}

pub fn go_live(control: &VoiceControl, fixtures: &mut Fixtures, per_channel: usize) {
	if per_channel == 0 {
		return;
	}
	for guild in &mut fixtures.guilds {
		let mut live: HashMap<u64, usize> = HashMap::new();
		for state in &mut guild.voice_states {
			let Some(channel_id) = state.channel_id else {
				continue;
			};
			let count = live.entry(channel_id.0).or_default();
			if state.user_id.0 == CURRENT_USER_ID || *count >= per_channel {
				continue;
			}
			*count += 1;
			state.self_stream = Some(true);
			control.start_stream(Some(guild.id), channel_id.0, state.user_id.0);
		}
	}
}

fn stream_key(guild_id: Option<u64>, channel_id: u64, owner: u64) -> String {
	match guild_id {
		Some(guild_id) => format!("guild:{guild_id}:{channel_id}:{owner}"),
		None => format!("call:{channel_id}:{owner}"),
	}
}

impl Voice {
	pub(super) fn announce_streams(&self, peer: &Peer) {
		let Some((_, channel_id)) = self.member_of(peer.session_id.as_deref()) else {
			return;
		};
		let stream = self
			.streams
			.lock()
			.playing
			.values()
			.find(|playing| playing.stream.rtc_channel_id == channel_id && playing.task.is_some())
			.map(|playing| playing.stream.clone());
		let Some(stream) = stream else {
			return;
		};
		let owner = stream.owner.to_string();
		let connected = json!({ "op": 11, "d": { "user_ids": [owner] } });
		let video = json!({
			"op": 12,
			"d": {
				"user_id": owner,
				"audio_ssrc": stream.audio_ssrc(),
				"video_ssrc": stream.video_ssrc,
				"streams": [{
					"type": "video",
					"ssrc": stream.video_ssrc,
					"rtx_ssrc": stream.video_ssrc + 1,
					"rid": "100",
					"quality": 100,
					"active": true,
				}],
			},
		});
		for frame in [connected, video] {
			let _ = self.broadcasts.send((SCRIPTED, Some(channel_id), frame));
		}
	}
}

impl VoiceControl {
	pub fn start_stream(&self, guild_id: Option<u64>, channel_id: u64, owner: u64) -> MockStream {
		if let Some(stream) = self.stream(&stream_key(guild_id, channel_id, owner)) {
			return stream;
		}
		let stream = self.open_stream(guild_id, channel_id, owner);
		if self.0.settings.lock().dave_version > 0 {
			self.add_member(stream.rtc_channel_id, owner);
		}
		let task = ScriptedCamera::new(self.clone(), stream.rtc_channel_id, owner, stream.video_ssrc).play();
		if let Some(playing) = self.0.streams.lock().playing.get_mut(&stream.key) {
			playing.task = Some(task);
		}
		stream
	}

	pub fn own_stream(&self, guild_id: Option<u64>, channel_id: u64) -> MockStream {
		self.open_stream(guild_id, channel_id, CURRENT_USER_ID)
	}

	pub fn streaming_in(&self, channel_id: u64, owner: u64) -> bool {
		self.0
			.streams
			.lock()
			.playing
			.values()
			.any(|playing| playing.stream.channel_id == channel_id && playing.stream.owner == owner)
	}

	fn open_stream(&self, guild_id: Option<u64>, channel_id: u64, owner: u64) -> MockStream {
		let key = stream_key(guild_id, channel_id, owner);
		if let Some(stream) = self.stream(&key) {
			return stream;
		}
		let index = {
			let mut streams = self.0.streams.lock();
			streams.started += 1;
			streams.started - 1
		};
		let rtc_server_id = FIRST_RTC_SERVER_ID + index * RTC_ID_STRIDE;
		let stream = MockStream {
			key: key.clone(),
			owner,
			guild_id,
			channel_id,
			rtc_server_id,
			rtc_channel_id: rtc_server_id + 1,
			video_ssrc: FIRST_STREAM_SSRC + index as u32 * STREAM_SSRC_STRIDE,
			full: false,
		};
		self.0
			.rooms
			.lock()
			.set_group(stream.rtc_channel_id, stream.dave_group());
		self.0.streams.lock().playing.insert(
			key,
			Playing {
				stream: stream.clone(),
				task: None,
			},
		);
		stream
	}

	pub fn stream(&self, key: &str) -> Option<MockStream> {
		self.0
			.streams
			.lock()
			.playing
			.get(key)
			.map(|playing| playing.stream.clone())
	}

	pub fn streams(&self) -> Vec<MockStream> {
		let mut streams: Vec<MockStream> = self
			.0
			.streams
			.lock()
			.playing
			.values()
			.map(|playing| playing.stream.clone())
			.collect();
		streams.sort_by_key(|stream| stream.rtc_server_id);
		streams
	}

	pub fn fill_stream(&self, key: &str) {
		if let Some(playing) = self.0.streams.lock().playing.get_mut(key) {
			playing.stream.full = true;
		}
	}

	pub fn end_stream(&self, key: &str) -> Option<MockStream> {
		let playing = self.0.streams.lock().playing.remove(key)?;
		if let Some(task) = playing.task {
			task.abort();
			self.disconnect_in(playing.stream.rtc_channel_id, playing.stream.owner);
		}
		Some(playing.stream)
	}

	fn disconnect_in(&self, channel_id: u64, user_id: u64) {
		let frame = json!({ "op": 13, "d": { "user_id": user_id.to_string() } });
		let _ = self.0.broadcasts.send((SCRIPTED, Some(channel_id), frame));
	}
}
