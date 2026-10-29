use std::sync::Arc;
use std::time::{Duration, Instant};

use mockcord::{GatewayFrame, MockStream, VoiceFrame, VoicePacket, VoiceRecording, VoiceVideoFrame};
use opus::{Channels, Decoder};
use protocol::{Opcode, Snowflake};
use serde_json::{Value, json};
use session::{Command, FAKE_SOUND_HZ, FAKE_SOUND_LEVEL, FakeCapture, VoiceSetup};
use state::State;
use voice::video::{H264Decoder, ScreenKind, ScreenTarget, VideoQuality};
use voice::{SILENCE_FRAME, SoundTarget, VoiceConnection};

use crate::voice::{Voice, identifies, manual_setup, start};

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
const SPEAKING: i64 = 5;
const SOUNDSHARE: i64 = 2;
const SHARED_PROCESS: u32 = 4_100;
const SOUND_PACKETS: usize = 25;
const SOUND_SKIPPED: usize = 5;
const STEREO_FRAME: usize = 1_920;
const TONE_SHARE: f32 = 0.9;
const LEVEL_SLACK_DB: f32 = 3.0;
const SWAPPED_SIZE: (u32, u32) = (1_920, 1_080);

fn shared_window() -> ScreenTarget {
	ScreenTarget {
		kind: ScreenKind::Window,
		id: 2,
		name: "Kyoko".to_owned(),
		width: 1_600,
		height: 900,
		process: SHARED_PROCESS,
	}
}

fn shared_screen() -> ScreenTarget {
	ScreenTarget {
		kind: ScreenKind::Screen,
		id: 1,
		name: "Screen 1".to_owned(),
		width: 1_920,
		height: 1_080,
		process: 0,
	}
}

fn full_hd() -> VideoQuality {
	VideoQuality {
		width: 1_920,
		height: 1_080,
		fps: 15,
		bitrate: 4_000_000,
	}
}

fn sound_after(recording: &VoiceRecording, ssrc: u32, after: Instant) -> Vec<VoicePacket> {
	recording
		.packets_from(ssrc)
		.filter(|packet| packet.at > after && packet.payload != SILENCE_FRAME)
		.cloned()
		.collect()
}

fn decoded_sound(packets: &[VoicePacket]) -> Vec<f32> {
	let mut decoder = Decoder::new(48_000, Channels::Stereo).expect("an opus decoder");
	let mut frame = vec![0.0f32; STEREO_FRAME];
	let mut samples = Vec::new();
	for packet in packets.iter().skip(SOUND_SKIPPED) {
		let opened = packet.opened.as_deref().expect("the viewer opens the sound");
		let decoded = decoder
			.decode_float(opened, &mut frame, false)
			.expect("the sound decodes");
		samples.extend(
			frame[..decoded * 2]
				.as_chunks::<2>()
				.0
				.iter()
				.map(|[left, right]| (left + right) * 0.5),
		);
	}
	samples
}

fn tone_share(samples: &[f32], hz: f32) -> f32 {
	let step = hz * std::f32::consts::TAU / 48_000.0;
	let (sine, cosine) =
		samples
			.iter()
			.enumerate()
			.fold((0.0f32, 0.0f32), |(sine, cosine), (index, sample)| {
				let phase = index as f32 * step;
				(sine + sample * phase.sin(), cosine + sample * phase.cos())
			});
	let tone = 2.0 * (sine * sine + cosine * cosine) / samples.len() as f32;
	let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
	tone / energy.max(f32::EPSILON)
}

fn level_db(samples: &[f32]) -> f32 {
	let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
	10.0 * (energy / samples.len().max(1) as f32 + 1e-12).log10()
}

fn soundshare_flags(recording: &VoiceRecording, connection: u64, after: Instant) -> Vec<i64> {
	recording
		.frames_with(SPEAKING)
		.filter(|frame| frame.connection == connection && frame.at > after)
		.filter_map(|frame| frame.data["speaking"].as_i64())
		.collect()
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

	async fn sound_packets(&self, ssrc: u32, after: Instant, what: &str) -> Vec<VoicePacket> {
		let recording = self
			.voice_until(what, |recording| {
				sound_after(recording, ssrc, after).len() >= SOUND_PACKETS + SOUND_SKIPPED
			})
			.await;
		sound_after(&recording, ssrc, after)
	}

	async fn sound_goes_quiet(&self, ssrc: u32, why: &str) {
		tokio::time::sleep(DRAIN).await;
		let quiet_from = Instant::now();
		tokio::time::sleep(QUIET).await;
		assert!(
			sound_after(&self.running.voice().recording(), ssrc, quiet_from).is_empty(),
			"{why}"
		);
	}

	fn fake_capture(&self) -> FakeCapture {
		let capture = FakeCapture::default();
		self.running.session().set_voice_setup(VoiceSetup {
			capture: capture.clone(),
			..manual_setup(&self.running)
		});
		capture
	}

	async fn live_with_a_viewer(&self, sound: bool) -> (MockStream, Arc<VoiceConnection>, Instant) {
		let control = self.running.voice();
		control.set_dave_version(1);
		control.add_member(self.lounge.get(), MEMBER);
		self.join(self.lounge);
		self.connected_to(self.lounge).await;
		let session = self.running.session();
		self.running
			.eventually("the call's DAVE group to form", || {
				control.dave_group(self.lounge.get()).len() == 2
					&& session
						.voice_connection()
						.is_some_and(|connection| connection.dave().enabled())
			})
			.await;
		let asked = self.command(Command::GoLive {
			source: shared_window(),
			quality: quality(),
			sound,
		});
		let stream = self.own_stream().await;
		control.add_member(stream.rtc_channel_id, VIEWER);
		self.running
			.eventually("the go live connection to come up", || {
				session.share_connection().is_some()
			})
			.await;
		let connection = session.share_connection().expect("the go live connection");
		self.running
			.eventually("the stream's DAVE group to form", || {
				control.dave_group(stream.rtc_channel_id).len() == 2 && connection.dave().enabled()
			})
			.await;
		(stream, connection, asked)
	}

	fn stream_socket(&self, stream: &MockStream) -> u64 {
		identifies(&self.running.voice().recording())
			.into_iter()
			.find(|frame| frame.data["server_id"] == json!(stream.rtc_server_id.to_string()))
			.expect("the stream identify")
			.connection
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
		sound: false,
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
		sound: false,
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
		sound: false,
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

#[tokio::test(flavor = "multi_thread")]
async fn going_live_shares_the_windows_sound_dave_encrypted_and_minimising_pauses_it() {
	let voice = start("go-live-sound").await;
	let capture = voice.fake_capture();
	let session = voice.running.session();
	let me = me(&voice);
	let (stream, connection, asked) = voice.live_with_a_viewer(true).await;
	let socket = voice.stream_socket(&stream);
	let ssrc = screen_ssrc(&voice.running.voice().recording(), socket);
	let audio_ssrc = connection.ssrc().expect("the stream connection's ssrc");
	voice
		.running
		.eventually("the window's sound to open", || {
			!capture.sound_targets().is_empty()
		})
		.await;
	assert_eq!(
		capture.sound_targets(),
		[SoundTarget::App {
			process: SHARED_PROCESS
		}],
		"an app shares only its own process tree's sound"
	);

	let encrypted = Instant::now();
	let sound = voice
		.sound_packets(audio_ssrc, encrypted, "the stream's sound")
		.await;
	for packet in &sound {
		assert!(packet.payload.ends_with(&DAVE_MARKER), "a DAVE frame");
		let opened = packet.opened.as_deref().expect("the viewer opens the sound");
		assert_ne!(opened, packet.payload.as_slice(), "the sound was encrypted");
	}
	let heard = decoded_sound(&sound);
	let share = tone_share(&heard, FAKE_SOUND_HZ);
	let level = level_db(&heard);
	let expected = 20.0 * (FAKE_SOUND_LEVEL / std::f32::consts::SQRT_2).log10();
	eprintln!("stream sound: {share:.3} of it the tone at {level:.1} dB (sent at {expected:.1} dB)");
	assert!(
		share > TONE_SHARE,
		"the viewer hears the shared sound: {share:.3}"
	);
	assert!(
		(level - expected).abs() <= LEVEL_SLACK_DB,
		"the sound keeps its level, no voice processing: {level:.1} dB against {expected:.1} dB"
	);
	assert!(
		soundshare_flags(&voice.running.voice().recording(), socket, asked).contains(&SOUNDSHARE),
		"op 5 announces the sound as soundshare"
	);
	assert_eq!(
		connection.stats().frames_echo_cancelled + connection.stats().frames_denoised,
		0,
		"stream sound skips the microphone's processing"
	);
	voice
		.running
		.eventually("your own stream to preview from local frames", || {
			session.stream_feed().decoded(me) >= STREAM_FRAMES as u64
		})
		.await;

	let minimised = Instant::now();
	capture.set_minimised(true);
	let op_22 = voice
		.gateway_frame(Opcode::StreamSetPaused, minimised, "op 22 on minimise", |data| {
			data["paused"] == json!(true)
		})
		.await;
	assert_eq!(op_22.data["stream_key"], json!(stream.key));
	voice
		.running
		.until("your stream to show as paused", |state| {
			state.stream_of(me).is_some_and(|summary| summary.paused)
		})
		.await;
	voice
		.screen_goes_quiet(ssrc, "a minimised window sends no video")
		.await;
	voice
		.sound_goes_quiet(audio_ssrc, "a minimised window sends no sound")
		.await;
	assert!(
		soundshare_flags(&voice.running.voice().recording(), socket, minimised).contains(&0),
		"op 5 ends the soundshare while paused"
	);

	let restored = Instant::now();
	capture.set_minimised(false);
	voice
		.gateway_frame(Opcode::StreamSetPaused, restored, "op 22 on restore", |data| {
			data["paused"] == json!(false)
		})
		.await;
	let frames = voice
		.screen_frames(ssrc, restored, "the screen after restoring")
		.await;
	assert!(frames[0].is_keyframe(), "a restored window resumes on a keyframe");
	voice
		.sound_packets(audio_ssrc, restored, "the sound after restoring")
		.await;

	voice.command(Command::StopGoLive);
	voice
		.running
		.eventually("the own preview to clear", || {
			session.share_connection().is_none() && session.stream_feed().decoded(me) == 0
		})
		.await;
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_windows_swaps_the_capture_on_the_same_stream_connection_and_key() {
	let voice = start("go-live-swap").await;
	let capture = voice.fake_capture();
	let session = voice.running.session();
	let control = voice.running.voice();
	let (stream, connection, asked) = voice.live_with_a_viewer(true).await;
	let socket = voice.stream_socket(&stream);
	let ssrc = screen_ssrc(&control.recording(), socket);
	let frames = voice.screen_frames(ssrc, asked, "the window's frames").await;
	assert_eq!(decoded_size(&frames), Some(SENT_SIZE));

	let swapped = voice.command(Command::GoLive {
		source: shared_screen(),
		quality: full_hd(),
		sound: true,
	});
	let recording = voice
		.voice_until("op 12 announcing the new size", |recording| {
			video_states(recording, socket, swapped)
				.iter()
				.any(|state| state.data["streams"][0]["max_resolution"]["width"] == json!(SWAPPED_SIZE.0))
		})
		.await;
	let announced = video_states(&recording, socket, swapped)
		.into_iter()
		.find(|state| state.data["streams"][0]["max_resolution"]["width"] == json!(SWAPPED_SIZE.0))
		.expect("the new size");
	assert_eq!(announced.data["video_ssrc"], json!(ssrc), "the same layer");
	assert_eq!(
		announced.data["streams"][0]["max_resolution"]["height"],
		json!(SWAPPED_SIZE.1)
	);
	let frames = voice.screen_frames(ssrc, swapped, "the screen's frames").await;
	let first = frames
		.iter()
		.position(VoiceVideoFrame::is_keyframe)
		.expect("a keyframe after the swap");
	for frame in &frames[first..] {
		assert!(frame.opened.is_some(), "the viewer still opens the frames");
	}
	voice
		.running
		.eventually("frames at the new size", || {
			decoded_size(&frames_after(&control.recording(), ssrc, swapped)) == Some(SWAPPED_SIZE)
		})
		.await;
	voice
		.running
		.eventually("the screen's sound", || capture.sound_targets().len() == 2)
		.await;
	assert_eq!(
		capture.sound_targets(),
		[
			SoundTarget::App {
				process: SHARED_PROCESS
			},
			SoundTarget::Everything {
				except: std::process::id()
			}
		],
		"a screen shares everything but kyoko's own sound"
	);
	voice
		.sound_packets(
			connection.ssrc().expect("an ssrc"),
			Instant::now(),
			"the screen's sound",
		)
		.await;

	let still = session.share_connection().expect("the stream connection");
	assert!(Arc::ptr_eq(&still, &connection), "the same stream connection");
	let gateway = voice.running.mock().recording();
	assert_eq!(
		gateway.frames_with(Opcode::StreamCreate).count(),
		1,
		"no second op 18"
	);
	assert_eq!(gateway.frames_with(Opcode::StreamDelete).count(), 0, "no op 19");
	let recording = control.recording();
	assert_eq!(
		identifies(&recording)
			.into_iter()
			.filter(|frame| frame.data["server_id"] == json!(stream.rtc_server_id.to_string()))
			.count(),
		1,
		"the stream connection never reconnected"
	);
	assert!(
		!recording.closes.iter().any(|(closed, _)| *closed == socket),
		"the stream socket stayed open"
	);
	voice
		.running
		.until("the same stream key", |state| {
			sharing(state).as_deref() == Some(stream.key.as_str())
		})
		.await;
	voice.stop().await;
}
