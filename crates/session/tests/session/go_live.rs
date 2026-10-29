use std::sync::Arc;
use std::time::{Duration, Instant};

use mockcord::{GatewayFrame, MockStream, VoiceFrame, VoiceRecording, VoiceVideoFrame};
use protocol::{Opcode, Snowflake};
use serde_json::{Value, json};
use session::Command;
use state::State;
use voice::video::{H264Decoder, ScreenKind, ScreenTarget, VideoQuality};

use crate::voice::{Voice, identifies, start};

const READY: i64 = 2;
const VIDEO: i64 = 12;
const MEMBER: u64 = 222_801_430_442_967_040;
const VIEWER: u64 = 333_014_820_093_313_024;
const DAVE_MARKER: [u8; 2] = [0xfa, 0xfa];
const STREAM_FRAMES: usize = 6;
const QUIET_FRAMES: usize = 1;
const DRAIN: Duration = Duration::from_millis(300);
const QUIET: Duration = Duration::from_millis(600);
const SENT_SIZE: (u32, u32) = (1_280, 720);
const OP_18_KEYS: [&str; 4] = ["type", "guild_id", "channel_id", "preferred_region"];
const OP_22_KEYS: [&str; 2] = ["stream_key", "paused"];

fn shared_window() -> ScreenTarget {
	ScreenTarget {
		kind: ScreenKind::Window,
		id: 2,
		name: "Kyoko".to_owned(),
		width: 1_600,
		height: 900,
	}
}

fn quality() -> VideoQuality {
	VideoQuality {
		width: 1_280,
		height: 720,
		fps: 30,
		bitrate: 2_500_000,
	}
}

fn me(voice: &Voice) -> Snowflake {
	voice
		.running
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("signed in")
}

fn live(state: &State, guild_id: Snowflake, user_id: Snowflake) -> bool {
	state
		.guild(guild_id)
		.and_then(|guild| guild.voice_states.get(&user_id))
		.is_some_and(|voice| voice.self_stream == Some(true))
}

fn sharing(state: &State) -> Option<String> {
	state
		.voice()
		.and_then(|voice| voice.screen_share.as_ref())
		.map(|share| share.stream_key.clone())
}

fn screen_ssrc(recording: &VoiceRecording, connection: u64) -> u32 {
	let ready = recording
		.sent_with(READY)
		.find(|frame| frame.connection == connection)
		.expect("mockcord sent the stream connection its ready");
	let streams = ready.data["streams"].as_array().expect("ready lists streams");
	assert_eq!(streams.len(), 1, "a stream connection gets one layer");
	streams[0]["ssrc"]
		.as_u64()
		.and_then(|ssrc| u32::try_from(ssrc).ok())
		.expect("the screen layer's ssrc")
}

fn video_states(recording: &VoiceRecording, connection: u64, after: Instant) -> Vec<&VoiceFrame> {
	recording
		.frames_with(VIDEO)
		.filter(|frame| frame.connection == connection && frame.at > after)
		.collect()
}

fn frames_after(recording: &VoiceRecording, ssrc: u32, after: Instant) -> Vec<VoiceVideoFrame> {
	recording
		.video_from(ssrc)
		.filter(|frame| frame.at > after)
		.cloned()
		.collect()
}

fn decoded_size(frames: &[VoiceVideoFrame]) -> Option<(u32, u32)> {
	let mut decoder = H264Decoder::new().expect("Media Foundation decodes h264 here");
	let first = frames.iter().position(VoiceVideoFrame::is_keyframe)?;
	frames[first..].iter().find_map(|frame| {
		let opened = frame.opened.as_deref()?;
		let picture = decoder.decode(opened, Duration::ZERO).ok().flatten()?;
		Some((picture.width, picture.height))
	})
}

impl Voice {
	async fn gateway_frame(
		&self,
		op: Opcode,
		after: Instant,
		what: &str,
		wanted: impl Fn(&Value) -> bool,
	) -> GatewayFrame {
		self.running
			.until_wire(what, |recording| {
				recording
					.frames_with(op)
					.find(|frame| frame.at > after && wanted(&frame.data))
					.cloned()
			})
			.await
	}

	async fn own_stream(&self) -> MockStream {
		let key = format!("guild:{}:{}:{}", self.guild_id, self.lounge, me(self));
		let control = self.running.voice();
		self.running
			.eventually("mockcord to open your stream", || control.stream(&key).is_some())
			.await;
		control.stream(&key).expect("your stream")
	}

	async fn screen_frames(&self, ssrc: u32, after: Instant, what: &str) -> Vec<VoiceVideoFrame> {
		let recording = self
			.voice_until(what, |recording| {
				frames_after(recording, ssrc, after).len() >= STREAM_FRAMES
			})
			.await;
		frames_after(&recording, ssrc, after)
	}

	async fn screen_goes_quiet(&self, ssrc: u32, why: &str) {
		tokio::time::sleep(DRAIN).await;
		let quiet_from = Instant::now();
		tokio::time::sleep(QUIET).await;
		assert!(
			frames_after(&self.running.voice().recording(), ssrc, quiet_from).len() < QUIET_FRAMES,
			"{why}"
		);
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn going_live_sends_dave_h264_on_one_screen_layer_honours_wants_pauses_and_stops() {
	let voice = start("go-live").await;
	let control = voice.running.voice();
	control.set_dave_version(1);
	control.add_member(voice.lounge.get(), MEMBER);
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let session = voice.running.session();
	voice
		.running
		.eventually("the call's DAVE group to form", || {
			control.dave_group(voice.lounge.get()).len() == 2
				&& session
					.voice_connection()
					.is_some_and(|connection| connection.dave().enabled())
		})
		.await;
	let parent = session.voice_connection().expect("the call's connection");
	let audio_ssrc = voice.ssrc();
	let me = me(&voice);

	let asked = voice.command(Command::GoLive {
		source: shared_window(),
		quality: quality(),
	});
	let op_18 = voice
		.gateway_frame(Opcode::StreamCreate, asked, "op 18", |_| true)
		.await;
	assert_eq!(op_18.keys(&[]), OP_18_KEYS, "op 18 keys in the docs' order");
	assert_eq!(op_18.data["type"], json!("guild"));
	assert_eq!(op_18.data["guild_id"], json!(voice.guild_id.to_string()));
	assert_eq!(op_18.data["channel_id"], json!(voice.lounge.to_string()));
	assert_eq!(op_18.data["preferred_region"], json!("mock-east"));
	let stream = voice.own_stream().await;
	control.add_member(stream.rtc_channel_id, VIEWER);

	voice
		.running
		.eventually("the go live connection to come up", || {
			session.share_connection().is_some()
		})
		.await;
	let connection = session.share_connection().expect("the go live connection");
	let recording = control.recording();
	let identify = identifies(&recording)
		.into_iter()
		.find(|frame| frame.data["server_id"] == json!(stream.rtc_server_id.to_string()))
		.expect("the stream identify")
		.clone();
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
		json!([{ "type": "screen", "rid": "100", "quality": 100 }])
	);
	let socket = identify.connection;
	let ssrc = screen_ssrc(&recording, socket);
	voice
		.running
		.eventually("the stream's DAVE group to form", || {
			control.dave_group(stream.rtc_channel_id).len() == 2 && connection.dave().enabled()
		})
		.await;

	let recording = voice
		.voice_until("op 12 announcing the screen", |recording| {
			!video_states(recording, socket, asked).is_empty()
		})
		.await;
	let announced = video_states(&recording, socket, asked)[0];
	assert_eq!(
		announced.data["audio_ssrc"],
		json!(connection.ssrc().expect("an ssrc"))
	);
	assert_eq!(announced.data["video_ssrc"], json!(ssrc));
	let layer = &announced.data["streams"][0];
	assert_eq!(layer["type"], json!("screen"));
	assert_eq!(layer["rid"], json!("100"));
	assert_eq!(layer["quality"], json!(100));
	assert_eq!(layer["active"], json!(true));
	assert_eq!(layer["max_resolution"]["width"], json!(SENT_SIZE.0));
	assert_eq!(layer["max_resolution"]["height"], json!(SENT_SIZE.1));
	assert!(
		video_states(&recording, parent_socket(&recording, &voice), asked).is_empty(),
		"the call announces no video"
	);

	let frames = voice.screen_frames(ssrc, asked, "screen frames").await;
	assert!(frames[0].is_keyframe(), "the stream starts on a keyframe");
	for frame in &frames {
		assert!(frame.complete);
		assert!(frame.sealed.ends_with(&DAVE_MARKER), "a DAVE frame");
		let opened = frame.opened.as_deref().expect("the viewer opens the frame");
		assert_ne!(opened, frame.sealed.as_slice(), "the frame was encrypted");
		assert_eq!(frame.rid().as_deref(), Some("100"));
	}
	assert_eq!(decoded_size(&frames), Some(SENT_SIZE));
	voice
		.running
		.until("your LIVE voice state", |state| {
			live(state, voice.guild_id, me) && sharing(state).as_deref() == Some(stream.key.as_str())
		})
		.await;

	control.sink_wants(json!({ ssrc.to_string(): 0, "any": 100 }));
	let unwanted = Instant::now();
	voice
		.voice_until("op 12 marking the screen inactive", |recording| {
			video_states(recording, socket, unwanted)
				.iter()
				.any(|state| state.data["streams"][0]["active"] == json!(false))
		})
		.await;
	voice.screen_goes_quiet(ssrc, "no viewer wants the screen").await;
	control.sink_wants(json!({ ssrc.to_string(): 100, "any": 100 }));
	let wanted = Instant::now();
	let frames = voice.screen_frames(ssrc, wanted, "the screen again").await;
	assert!(frames[0].is_keyframe(), "a wanted screen resumes on a keyframe");

	let paused = voice.command(Command::SetStreamPaused(true));
	let op_22 = voice
		.gateway_frame(Opcode::StreamSetPaused, paused, "op 22 pausing", |data| {
			data["paused"] == json!(true)
		})
		.await;
	assert_eq!(op_22.keys(&[]), OP_22_KEYS);
	assert_eq!(op_22.data["stream_key"], json!(stream.key));
	voice
		.running
		.until("the stream to show as paused", |state| {
			state.stream_of(me).is_some_and(|summary| summary.paused)
		})
		.await;
	voice
		.screen_goes_quiet(ssrc, "a paused stream sends no video")
		.await;
	let resumed = voice.command(Command::SetStreamPaused(false));
	voice
		.gateway_frame(Opcode::StreamSetPaused, resumed, "op 22 resuming", |data| {
			data["paused"] == json!(false)
		})
		.await;
	let frames = voice
		.screen_frames(ssrc, resumed, "the screen after the pause")
		.await;
	assert!(frames[0].is_keyframe(), "a resumed stream starts on a keyframe");

	let stopped = voice.command(Command::StopGoLive);
	voice
		.gateway_frame(Opcode::StreamDelete, stopped, "op 19", |data| {
			data["stream_key"] == json!(stream.key)
		})
		.await;
	voice
		.running
		.eventually("the go live connection to close", || {
			session.share_connection().is_none()
				&& control
					.recording()
					.closes
					.iter()
					.any(|(closed, _)| *closed == socket)
		})
		.await;
	voice
		.running
		.until("your LIVE badge to go", |state| {
			!live(state, voice.guild_id, me) && sharing(state).is_none()
		})
		.await;
	voice.screen_goes_quiet(ssrc, "no video after stopping").await;
	let still = session.voice_connection().expect("the call stays connected");
	assert!(Arc::ptr_eq(&still, &parent), "the call's connection is untouched");
	let recording = control.recording();
	let call_socket = parent_socket(&recording, &voice);
	assert!(
		!recording.closes.iter().any(|(closed, _)| *closed == call_socket),
		"the call's socket never closed"
	);
	assert_eq!(control.dave_group(voice.lounge.get()).len(), 2);
	voice.speaks(audio_ssrc, "audio after going live").await;
	voice.stop().await;
}

fn parent_socket(recording: &VoiceRecording, voice: &Voice) -> u64 {
	identifies(recording)
		.into_iter()
		.find(|frame| frame.data["server_id"] == json!(voice.guild_id.to_string()))
		.expect("the call's identify")
		.connection
}

#[tokio::test(flavor = "multi_thread")]
async fn going_live_needs_a_call_and_leaving_the_call_ends_the_stream() {
	let voice = start("go-live-leave").await;
	voice.command(Command::GoLive {
		source: shared_window(),
		quality: quality(),
	});
	voice
		.running
		.until("going live outside a call to be refused", |state| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.reason == "Join a voice channel to go live.")
		})
		.await;
	assert!(
		voice
			.running
			.mock()
			.recording()
			.frames_with(Opcode::StreamCreate)
			.next()
			.is_none(),
		"no op 18 outside a call"
	);

	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let session = voice.running.session();
	voice.command(Command::GoLive {
		source: shared_window(),
		quality: quality(),
	});
	let stream = voice.own_stream().await;
	voice
		.running
		.eventually("the go live connection to come up", || {
			session.share_connection().is_some()
		})
		.await;
	let left = voice.command(Command::LeaveVoice);
	voice
		.gateway_frame(Opcode::StreamDelete, left, "op 19 on leaving", |data| {
			data["stream_key"] == json!(stream.key)
		})
		.await;
	voice
		.running
		.eventually("the go live connection to close", || {
			session.share_connection().is_none()
		})
		.await;
	voice
		.running
		.until("the call to end", |state| state.voice().is_none())
		.await;
	voice.stop().await;
}
