use std::time::{Duration, Instant};

use protocol::Snowflake;
use state::{State, VoiceStatus};

use crate::sidebar::{VoiceMember, VoiceVideo, voice_member};

pub const SPEAKING_PACE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTile {
	pub user_id: Snowflake,
	pub name: String,
	pub me: bool,
	pub muted: bool,
	pub deafened: bool,
	pub video: Option<VoiceVideo>,
	pub live: bool,
	pub volume: u8,
	pub local_muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallStream {
	pub user_id: Snowflake,
	pub name: String,
	pub stream_key: String,
	pub me: bool,
	pub watching: bool,
	pub paused: bool,
	pub volume: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCall {
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub name: String,
	pub connected: bool,
	pub tiles: Vec<CallTile>,
	pub streams: Vec<CallStream>,
}

pub fn stream_key(guild_id: Option<Snowflake>, channel_id: Snowflake, user_id: Snowflake) -> String {
	match guild_id {
		Some(guild_id) => format!("guild:{guild_id}:{channel_id}:{user_id}"),
		None => format!("call:{channel_id}:{user_id}"),
	}
}

pub fn voice_call(state: &State) -> Option<VoiceCall> {
	let session = state.voice()?;
	if matches!(session.status, VoiceStatus::Disconnected { .. }) {
		return None;
	}
	let channel_id = session.channel_id;
	let guild_id = session.guild_id;
	let me = state.current_user_id();
	let mut members: Vec<VoiceMember> = guild_id
		.and_then(|guild_id| state.guild(guild_id).map(|guild| (guild_id, guild)))
		.map(|(guild_id, guild)| {
			guild
				.voice_states
				.values()
				.filter(|voice| voice.channel_id == Some(channel_id))
				.map(|voice| voice_member(state, guild_id, voice))
				.collect()
		})
		.unwrap_or_default();
	let peers = session.peers.iter().copied().chain(me);
	for user_id in peers {
		if !members.iter().any(|member| member.user_id == user_id) {
			members.push(VoiceMember {
				user_id,
				name: state.display_name(guild_id, user_id),
				muted: false,
				deafened: false,
				video: None,
				live: false,
			});
		}
	}
	let mut tiles: Vec<CallTile> = members
		.into_iter()
		.map(|member| {
			let mine = Some(member.user_id) == me;
			let audio = state.user_audio(member.user_id);
			CallTile {
				user_id: member.user_id,
				me: mine,
				muted: member.muted || (mine && (session.self_mute || session.self_deaf)),
				deafened: member.deafened || (mine && session.self_deaf),
				video: if session.remote_video.contains_key(&member.user_id) || (mine && session.camera_on) {
					Some(VoiceVideo::Camera)
				} else if mine && member.video == Some(VoiceVideo::Camera) {
					None
				} else {
					member.video
				},
				live: member.live || (mine && session.screen_share.is_some()),
				volume: audio.volume,
				local_muted: !mine && audio.muted,
				name: member.name,
			}
		})
		.collect();
	tiles.sort_by_cached_key(|tile| (tile.name.to_lowercase(), tile.user_id));
	let streams = tiles
		.iter()
		.filter(|tile| tile.live && (!tile.me || session.screen_share.is_some()))
		.map(|tile| {
			let stream_key = stream_key(guild_id, channel_id, tile.user_id);
			CallStream {
				user_id: tile.user_id,
				name: tile.name.clone(),
				me: tile.me,
				watching: tile.me || session.watching.as_deref() == Some(stream_key.as_str()),
				paused: state.stream_of(tile.user_id).is_some_and(|stream| stream.paused),
				volume: state.user_audio(tile.user_id).stream_volume,
				stream_key,
			}
		})
		.collect();
	let name = state
		.channel(channel_id)
		.and_then(|channel| channel.name.clone())
		.unwrap_or_default();
	Some(VoiceCall {
		channel_id,
		guild_id,
		name,
		connected: session.status == VoiceStatus::Connected,
		tiles,
		streams,
	})
}

pub fn voice_speaking(state: &State) -> Vec<Snowflake> {
	let mut speaking: Vec<Snowflake> = state
		.voice()
		.map(|session| session.speaking.iter().copied().collect())
		.unwrap_or_default();
	speaking.sort_unstable();
	speaking
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pace {
	Publish(Vec<Snowflake>),
	Wait(Instant),
	Same,
}

#[derive(Debug, Default)]
pub struct SpeakingPace {
	shown: Vec<Snowflake>,
	published: Option<Instant>,
}

impl SpeakingPace {
	pub fn offer(&mut self, speaking: Vec<Snowflake>, now: Instant) -> Pace {
		if speaking == self.shown {
			return Pace::Same;
		}
		if let Some(published) = self.published {
			let due = published + SPEAKING_PACE;
			if now < due {
				return Pace::Wait(due);
			}
		}
		self.shown.clone_from(&speaking);
		self.published = Some(now);
		Pace::Publish(speaking)
	}

	pub fn shown(&self) -> &[Snowflake] {
		&self.shown
	}

	pub fn forget(&mut self) {
		*self = Self::default();
	}
}
