use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use mockcord::{CAMERA_HEIGHT, CAMERA_WIDTH, MockStream, VoiceControl, VoiceFrame, VoiceRecording};
use opus::{Application, Channels, Encoder};
use protocol::{Opcode, Snowflake};
use serde_json::json;
use session::{Command, VideoView};
use state::{State, UserAudio};
use tokio::task::JoinHandle;
use voice::{MixedFrame, VoiceConnection};

use crate::voice::{SETTLE, Voice, identifies, start, tone_frame};

const STREAM_FRAMES: u64 = 10;
const VIEW_WIDTH: u32 = 1_280;
const VIEW_HEIGHT: u32 = 720;
const DAVE_MEMBER: u64 = 222_801_430_442_967_040;
const OPUS_PACKET_BYTES: usize = 1_275;
const FRAME: Duration = Duration::from_millis(20);
const STREAM_AUDIO_FRAMES: usize = 60;
const WARMUP_FRAMES: usize = 5;
const MEASURED_FRAMES: usize = 25;
const HALF: u8 = 50;
const LOUD: u8 = 150;

fn seated(voice: &Voice) -> Vec<u64> {
	let me = voice.running.session().state().read().current_user_id();
	voice.running.fixtures().guilds[0]
		.voice_states
		.iter()
		.filter(|state| state.channel_id == Some(voice.lounge) && Some(state.user_id) != me)
		.map(|state| state.user_id.0)
		.collect()
}

fn live(state: &State, guild_id: Snowflake, user_id: u64) -> bool {
	state
		.guild(guild_id)
		.and_then(|guild| guild.voice_states.get(&Snowflake(user_id)))
		.is_some_and(|voice| voice.self_stream == Some(true))
}

fn watching(state: &State) -> Option<String> {
	state.voice().and_then(|voice| voice.watching.clone())
}

fn rejected_with(state: &State, text: &str) -> bool {
	state
		.last_rejection()
		.is_some_and(|rejection| rejection.reason == text)
}

fn stream_identify<'a>(recording: &'a VoiceRecording, stream: &MockStream) -> Option<&'a VoiceFrame> {
	identifies(recording)
		.into_iter()
		.find(|frame| frame.data["server_id"] == json!(stream.rtc_server_id.to_string()))
}

impl Voice {
	async fn go_live(&self, owner: u64) -> MockStream {
		let stream = self
			.running
			.mock()
			.start_stream(self.guild_id.0, self.lounge.0, owner);
		let guild_id = self.guild_id;
		self.running
			.until("the streamer's LIVE voice state", |state| {
				live(state, guild_id, owner)
			})
			.await;
		stream
	}

	async fn watch(&self, stream: &MockStream) {
		self.command(Command::WatchStream {
			stream_key: stream.key.clone(),
		});
		self.running
			.until_wire("op 20 for the stream", |recording| {
				recording
					.frames_with(Opcode::StreamWatch)
					.find(|frame| frame.data["stream_key"] == json!(stream.key))
					.cloned()
			})
			.await;
	}

	async fn stop_watching_sent(&self, stream: &MockStream) {
		self.running
			.until_wire("op 19 for the stream", |recording| {
				recording
					.frames_with(Opcode::StreamDelete)
					.find(|frame| frame.data["stream_key"] == json!(stream.key))
					.cloned()
			})
			.await;
	}

	async fn stream_connects(&self) {
		let session = self.running.session();
		self.running
			.eventually("the stream connection to come up", || {
				session.stream_connection().is_some()
			})
			.await;
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn watching_a_stream_connects_to_its_server_in_its_own_dave_group_and_leaves_the_call_alone() {
	let voice = start("streams-watch").await;
	let control = voice.running.voice();
	control.set_dave_version(1);
	control.add_member(voice.lounge.0, DAVE_MEMBER);
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let session = voice.running.session();
	voice
		.running
		.eventually("the call's DAVE group to form", || {
			control.dave_group(voice.lounge.0).len() == 2
				&& session
					.voice_connection()
					.is_some_and(|connection| connection.dave().enabled())
		})
		.await;
	let parent = session.voice_connection().expect("the call's connection");
	let owner = seated(&voice)[0];
	let stream = voice.go_live(owner).await;
	assert_eq!(stream.dave_group(), stream.rtc_server_id - 1);

	session.send(Command::SetStreamViews(vec![VideoView {
		user_id: Snowflake(owner),
		focused: true,
		width: VIEW_WIDTH,
		height: VIEW_HEIGHT,
	}]));
	voice.watch(&stream).await;
	voice
		.running
		.until("the stream to show as watched", |state| {
			watching(state).as_deref() == Some(stream.key.as_str())
		})
		.await;
	voice.stream_connects().await;

	voice
		.running
		.eventually("the stream identify", || {
			stream_identify(&control.recording(), &stream).is_some()
		})
		.await;
	let recording = control.recording();
	let identify = stream_identify(&recording, &stream).expect("the stream identify");
	assert_eq!(
		identify.data["channel_id"],
		json!(stream.rtc_channel_id.to_string())
	);
	assert_eq!(
		identify.data["token"],
		json!(format!("mock-voice-token-stream-{}", stream.rtc_server_id))
	);
	assert_eq!(
		identify.data["streams"],
		json!([{ "type": "screen", "rid": "100", "quality": 100 }]),
		"a stream connection offers one screen layer"
	);
	let parent_identify = identifies(&recording)
		.into_iter()
		.find(|frame| frame.data["server_id"] == json!(voice.guild_id.to_string()))
		.expect("the call's identify");
	assert_ne!(identify.connection, parent_identify.connection);
	assert_eq!(identify.data["session_id"], parent_identify.data["session_id"]);

	let connection = session.stream_connection().expect("the stream connection");
	voice
		.running
		.eventually("the stream's DAVE group to form", || {
			control.dave_group(stream.rtc_channel_id).len() == 2 && connection.dave().enabled()
		})
		.await;
	let feed = session.stream_feed();
	let owner_id = Snowflake(owner);
	voice
		.running
		.eventually("the streamed fixture to decode", || {
			feed.decoded(owner_id) >= STREAM_FRAMES
		})
		.await;
	assert_eq!(feed.source_size(owner_id), Some((CAMERA_WIDTH, CAMERA_HEIGHT)));
	assert!(connection.dave().decrypted() > 0, "stream frames open with DAVE");
	assert!(
		control
			.recording()
			.frames_with(15)
			.any(|frame| frame.connection == identify.connection
				&& frame.data[stream.video_ssrc.to_string()] == json!(100)),
		"the stream connection asks for the stream at full quality"
	);

	let still = session.voice_connection().expect("the call stays connected");
	assert!(Arc::ptr_eq(&still, &parent), "the call's connection is untouched");
	assert!(parent.dave().enabled());
	assert_eq!(control.dave_group(voice.lounge.0).len(), 2);
	assert!(!parent.video_users().contains(&owner_id));
	assert_eq!(session.video_feed().decoded(owner_id), 0);
	assert!(
		!control
			.recording()
			.closes
			.iter()
			.any(|(socket, _)| *socket == parent_identify.connection),
		"the call's socket never closed"
	);
	let own_ssrc = connection.ssrc().expect("the stream connection's ssrc");
	assert!(
		!control
			.recording()
			.packets
			.iter()
			.any(|packet| packet.ssrc == own_ssrc),
		"no media is sent to a stream you watch"
	);

	voice.command(Command::StopWatchingStream {
		stream_key: stream.key.clone(),
	});
	voice.stop_watching_sent(&stream).await;
	voice
		.running
		.until("the view to end", |state| watching(state).is_none())
		.await;
	assert!(session.stream_connection().is_none());
	assert!(
		session
			.voice_connection()
			.is_some_and(|now| Arc::ptr_eq(&now, &parent)),
		"stopping the stream keeps the call"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deletes_end_the_view_with_their_reason_and_leaving_voice_ends_every_view() {
	let voice = start("streams-delete").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let session = voice.running.session();
	let owners = seated(&voice);

	let full = voice.go_live(owners[0]).await;
	voice.running.mock().fill_stream(&full.key);
	voice.watch(&full).await;
	voice
		.running
		.until("a full stream to end the view", |state| {
			watching(state).is_none() && rejected_with(state, "This stream is full.")
		})
		.await;
	assert!(session.stream_connection().is_none());

	let ended = voice.go_live(owners[1]).await;
	voice.watch(&ended).await;
	voice.stream_connects().await;
	voice.running.mock().end_stream(&ended.key, "user_requested");
	voice
		.running
		.until("the streamer's stop to end the view", |state| {
			watching(state).is_none() && rejected_with(state, "The stream has ended.")
		})
		.await;
	voice
		.running
		.eventually("the stream connection to close", || {
			session.stream_connection().is_none()
		})
		.await;

	let again = voice.go_live(owners[1]).await;
	voice.watch(&again).await;
	voice.stream_connects().await;
	voice.command(Command::LeaveVoice);
	voice.stop_watching_sent(&again).await;
	voice
		.running
		.until("leaving to end the call and the view", |state| {
			state.voice().is_none()
		})
		.await;
	assert!(session.stream_connection().is_none());
	assert!(session.voice_connection().is_none());
	voice.stop().await;
}

fn play_stream_audio(control: VoiceControl, stream: &MockStream, from: u16) -> JoinHandle<()> {
	let (channel_id, owner, ssrc) = (stream.rtc_channel_id, stream.owner, stream.audio_ssrc());
	tokio::spawn(async move {
		let mut encoder = Encoder::new(48_000, Channels::Mono, Application::Audio).expect("an opus encoder");
		let mut packet = vec![0; OPUS_PACKET_BYTES];
		for index in 0..STREAM_AUDIO_FRAMES {
			let length = encoder
				.encode(&tone_frame(index), &mut packet)
				.expect("the tone encodes");
			let sequence = from.wrapping_add(index as u16);
			control.send_as(channel_id, owner, ssrc, sequence, &packet[..length]);
			tokio::time::sleep(FRAME).await;
		}
	})
}

fn level(frames: &[MixedFrame]) -> (f32, i16) {
	let samples: Vec<i16> = frames
		.iter()
		.skip(WARMUP_FRAMES)
		.take(MEASURED_FRAMES)
		.flat_map(|frame| frame.pcm.iter().copied())
		.collect();
	let energy: f64 = samples.iter().map(|sample| f64::from(*sample).powi(2)).sum();
	let rms = (energy / samples.len().max(1) as f64).sqrt() as f32;
	let peak = samples
		.iter()
		.map(|sample| sample.saturating_abs())
		.max()
		.unwrap_or(0);
	(rms, peak)
}

struct StreamAudio<'a> {
	voice: &'a Voice,
	stream: &'a MockStream,
	connection: Arc<VoiceConnection>,
	sequence: u16,
}

impl StreamAudio<'_> {
	fn play(&mut self) -> JoinHandle<()> {
		let sender = play_stream_audio(self.voice.running.voice(), self.stream, self.sequence);
		self.sequence = self.sequence.wrapping_add(STREAM_AUDIO_FRAMES as u16);
		sender
	}

	fn heard(&self) -> Vec<MixedFrame> {
		let ssrc = self.stream.audio_ssrc();
		self.connection
			.take_output()
			.into_iter()
			.filter(|frame| frame.sources.contains(&ssrc))
			.collect()
	}

	async fn level(&mut self) -> (f32, i16) {
		tokio::time::sleep(SETTLE).await;
		let _ = self.connection.take_output();
		let sender = self.play();
		let heard = RefCell::new(Vec::new());
		self.voice
			.running
			.eventually("the stream's audio to be mixed", || {
				let mut heard = heard.borrow_mut();
				heard.extend(self.heard());
				heard.len() >= WARMUP_FRAMES + MEASURED_FRAMES
			})
			.await;
		sender.await.expect("the stream audio was sent");
		level(&heard.into_inner())
	}

	async fn unheard(&mut self) -> usize {
		tokio::time::sleep(SETTLE).await;
		let _ = self.connection.take_output();
		self.play().await.expect("the stream audio was sent");
		tokio::time::sleep(SETTLE).await;
		self.heard().len()
	}
}

fn sent_by(control: &VoiceControl, ssrc: u32) -> usize {
	control
		.recording()
		.packets
		.iter()
		.filter(|packet| packet.ssrc == ssrc)
		.count()
}

fn both_volumes(state: &State, owner: Snowflake) -> bool {
	state.user_audio(owner)
		== UserAudio {
			volume: LOUD,
			muted: false,
			stream_volume: HALF,
		}
}

#[tokio::test(flavor = "multi_thread")]
async fn a_watched_streams_audio_plays_at_its_own_volume_sends_nothing_and_follows_deafen() {
	let mut voice = start("streams-audio").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let session = voice.running.session();
	let control = voice.running.voice();
	let owner = seated(&voice)[0];
	let owner_id = Snowflake(owner);
	let stream = voice.go_live(owner).await;
	voice.watch(&stream).await;
	voice.stream_connects().await;
	let connection = session.stream_connection().expect("the stream connection");
	voice
		.running
		.eventually("op 12 to name the streamer on the stream connection", || {
			connection.video_users().contains(&owner_id)
		})
		.await;
	let own_ssrc = connection.ssrc().expect("the stream connection's ssrc");
	let mut audio = StreamAudio {
		voice: &voice,
		stream: &stream,
		connection: connection.clone(),
		sequence: 1,
	};

	let (full, _) = audio.level().await;
	assert!(full > 0., "the stream's audio reaches the mixed output");

	voice.command(Command::SetStreamVolume {
		user_id: owner_id,
		volume: HALF,
	});
	voice
		.running
		.eventually("the stream volume to reach the stream's mixer", || {
			(connection.user_volume(owner_id) - 0.5).abs() < f32::EPSILON
		})
		.await;
	let (half, _) = audio.level().await;
	let ratio = half / full;
	assert!(
		(0.35..=0.65).contains(&ratio),
		"half stream volume halves the stream: {half} vs {full}"
	);

	voice.command(Command::SetUserVolume {
		user_id: owner_id,
		volume: LOUD,
	});
	let call = session.voice_connection().expect("the call's connection");
	voice
		.running
		.eventually("the voice volume to reach the call's mixer", || {
			(call.user_volume(owner_id) - 1.5).abs() < f32::EPSILON
		})
		.await;
	assert!(
		(connection.user_volume(owner_id) - 0.5).abs() < f32::EPSILON,
		"the voice volume leaves the stream alone"
	);

	voice.command(Command::SetSelfDeaf(true));
	voice
		.running
		.until("deafen to show", |state| {
			state.voice().is_some_and(|voice| voice.self_deaf)
		})
		.await;
	let (_, peak) = audio.level().await;
	assert_eq!(peak, 0, "deafen silences the stream");
	voice.command(Command::SetSelfDeaf(false));
	voice
		.running
		.until("undeafen to show", |state| {
			state.voice().is_some_and(|voice| !voice.self_deaf)
		})
		.await;
	let (back, _) = audio.level().await;
	assert!(back > 0., "undeafening plays the stream again");
	assert_eq!(
		sent_by(&control, own_ssrc),
		0,
		"the stream connection sends no media"
	);

	voice.command(Command::StopWatchingStream {
		stream_key: stream.key.clone(),
	});
	voice.stop_watching_sent(&stream).await;
	voice
		.running
		.eventually("the stream connection to close", || {
			session.stream_connection().is_none()
		})
		.await;
	assert_eq!(audio.unheard().await, 0, "stopping ends the stream's audio");
	assert_eq!(sent_by(&control, own_ssrc), 0);
	drop((audio, connection, call));

	voice
		.running
		.until("the stream volume beside the voice volume", |state| {
			both_volumes(state, owner_id)
		})
		.await;
	voice.running.restart().await;
	voice
		.running
		.until("both volumes after a restart", |state| {
			both_volumes(state, owner_id)
		})
		.await;
	voice.stop().await;
}
