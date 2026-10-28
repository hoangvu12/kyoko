use std::sync::Arc;

use mockcord::{CAMERA_HEIGHT, CAMERA_WIDTH, MockStream, VoiceFrame, VoiceRecording};
use protocol::{Opcode, Snowflake};
use serde_json::json;
use session::{Command, VideoView};
use state::State;

use crate::voice::{Voice, identifies, start};

const STREAM_FRAMES: u64 = 10;
const VIEW_WIDTH: u32 = 1_280;
const VIEW_HEIGHT: u32 = 720;
const DAVE_MEMBER: u64 = 222_801_430_442_967_040;

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
