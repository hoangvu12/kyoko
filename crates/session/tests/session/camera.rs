use std::time::{Duration, Instant};

use mockcord::{VoiceFrame, VoiceRecording, VoiceVideoFrame};
use serde_json::{Value, json};
use session::Command;
use voice::video::{H264Decoder, VideoCodecs};

use crate::voice::{Voice, for_channel, manual_setup, start};

const VIDEO: i64 = 12;
const READY: i64 = 2;
const MEMBER: u64 = 222_801_430_442_967_040;
const DAVE_MARKER: [u8; 2] = [0xfa, 0xfa];
const PLAYOUT_DELAY: [u8; 3] = [0, 0, 0];
const FRAMES_PER_LAYER: usize = 6;
const PAUSED_FRAMES: usize = 10;
const DRAIN: Duration = Duration::from_millis(300);
const VIDEO_STATE_KEYS: [&str; 4] = ["audio_ssrc", "video_ssrc", "rtx_ssrc", "streams"];
const FULL_SIZE: (u32, u32) = (1_280, 720);
const HALF_SIZE: (u32, u32) = (640, 360);

#[derive(Debug, Clone, Copy)]
struct Layer {
	ssrc: u32,
	rtx_ssrc: u32,
}

fn assigned(recording: &VoiceRecording, rid: &str) -> Layer {
	let ready = recording.sent_with(READY).last().expect("mockcord sent ready");
	let stream = ready.data["streams"]
		.as_array()
		.and_then(|streams| streams.iter().find(|stream| stream["rid"] == json!(rid)))
		.expect("ready assigns a stream for each offered layer");
	let number = |key: &str| stream[key].as_u64().and_then(|value| u32::try_from(value).ok());
	Layer {
		ssrc: number("ssrc").expect("an ssrc"),
		rtx_ssrc: number("rtx_ssrc").expect("an rtx ssrc"),
	}
}

fn video_states(recording: &VoiceRecording, after: Instant) -> Vec<&VoiceFrame> {
	recording
		.frames_with(VIDEO)
		.filter(|frame| frame.at > after)
		.collect()
}

fn stream<'a>(state: &'a VoiceFrame, rid: &str) -> &'a Value {
	state.data["streams"]
		.as_array()
		.and_then(|streams| streams.iter().find(|stream| stream["rid"] == json!(rid)))
		.unwrap_or(&Value::Null)
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

async fn join_with_member(name: &'static str) -> Voice {
	let voice = start(name).await;
	let control = voice.running.voice();
	control.set_dave_version(1);
	control.add_member(voice.lounge.get(), MEMBER);
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let session = voice.running.session();
	voice
		.running
		.eventually("DAVE to start with the member", || {
			control.dave_group(voice.lounge.get()).len() == 2
				&& session
					.voice_connection()
					.is_some_and(|connection| connection.dave().enabled())
		})
		.await;
	voice
}

fn camera_on(voice: &Voice) -> bool {
	voice
		.running
		.session()
		.state()
		.read()
		.voice()
		.is_some_and(|session| session.camera_on)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_camera_announces_both_layers_sends_dave_h264_honours_wants_and_clears() {
	let voice = join_with_member("camera-send").await;
	let control = voice.running.voice();
	let audio_ssrc = voice.ssrc();
	let recording = control.recording();
	let full = assigned(&recording, "100");
	let half = assigned(&recording, "50");

	let turned_on = voice.command(Command::SetSelfVideo(true));
	let op_4 = voice
		.op_4(turned_on, "op 4 with the camera on", |frame| {
			frame.data["self_video"] == json!(true)
		})
		.await;
	assert!(for_channel(&op_4, Some(voice.lounge)));
	let recording = voice
		.voice_until("op 12 announcing the camera", |recording| {
			!video_states(recording, turned_on).is_empty()
		})
		.await;
	let announced = video_states(&recording, turned_on)[0];
	assert_eq!(announced.keys(&["d"]), VIDEO_STATE_KEYS);
	assert_eq!(announced.data["audio_ssrc"], json!(audio_ssrc));
	assert_eq!(announced.data["video_ssrc"], json!(full.ssrc));
	assert_eq!(announced.data["rtx_ssrc"], json!(full.rtx_ssrc));
	for (rid, layer, size, quality) in [("100", full, FULL_SIZE, 100), ("50", half, HALF_SIZE, 50)] {
		let stream = stream(announced, rid);
		assert_eq!(stream["type"], json!("video"), "layer {rid}");
		assert_eq!(
			stream["ssrc"],
			json!(layer.ssrc),
			"layer {rid} uses the ready ssrc"
		);
		assert_eq!(stream["rtx_ssrc"], json!(layer.rtx_ssrc), "layer {rid}");
		assert_eq!(stream["quality"], json!(quality), "layer {rid}");
		assert_eq!(stream["active"], json!(true), "layer {rid}");
		assert_eq!(stream["max_resolution"]["width"], json!(size.0), "layer {rid}");
		assert_eq!(stream["max_resolution"]["height"], json!(size.1), "layer {rid}");
	}
	voice
		.running
		.until("the camera to show as on", |state| {
			state.voice().is_some_and(|session| session.camera_on)
		})
		.await;

	let recording = voice
		.voice_until("frames on both layers", |recording| {
			frames_after(recording, full.ssrc, turned_on).len() >= FRAMES_PER_LAYER
				&& frames_after(recording, half.ssrc, turned_on).len() >= FRAMES_PER_LAYER
		})
		.await;
	for (rid, layer, size) in [("100", full, FULL_SIZE), ("50", half, HALF_SIZE)] {
		let frames = frames_after(&recording, layer.ssrc, turned_on);
		assert!(frames[0].is_keyframe(), "layer {rid} starts on a keyframe");
		for frame in &frames {
			assert!(frame.complete, "layer {rid} arrives whole");
			assert!(
				frame.sealed.ends_with(&DAVE_MARKER),
				"layer {rid} is a DAVE frame"
			);
			let opened = frame.opened.as_deref().expect("the member opens the frame");
			assert_ne!(opened, frame.sealed.as_slice(), "layer {rid} was encrypted");
			assert_eq!(
				frame.extension(VoiceVideoFrame::PLAYOUT_DELAY),
				Some(PLAYOUT_DELAY.as_slice()),
				"layer {rid} carries playout-delay"
			);
			assert_eq!(frame.rid().as_deref(), Some(rid));
		}
		assert_eq!(
			decoded_size(&frames),
			Some(size),
			"layer {rid} decodes at its size"
		);
	}

	control.sink_wants(json!({ half.ssrc.to_string(): 0, "any": 100 }));
	let paused_at = Instant::now();
	let recording = voice
		.voice_until("op 12 marking the paused layer inactive", |recording| {
			video_states(recording, paused_at)
				.iter()
				.any(|state| stream(state, "50")["active"] == json!(false))
		})
		.await;
	let paused = video_states(&recording, paused_at)[0];
	assert_eq!(stream(paused, "100")["active"], json!(true));
	tokio::time::sleep(DRAIN).await;
	let quiet_from = Instant::now();
	voice
		.voice_until("the full layer to keep going", |recording| {
			frames_after(recording, full.ssrc, quiet_from).len() >= PAUSED_FRAMES
		})
		.await;
	assert!(
		frames_after(&control.recording(), half.ssrc, quiet_from).is_empty(),
		"wants of 0 pause that layer"
	);

	control.sink_wants(json!({ half.ssrc.to_string(): 50, "any": 100 }));
	let resumed_at = Instant::now();
	let recording = voice
		.voice_until("the half layer to resume", |recording| {
			!frames_after(recording, half.ssrc, resumed_at).is_empty()
		})
		.await;
	assert!(
		frames_after(&recording, half.ssrc, resumed_at)[0].is_keyframe(),
		"a resumed layer starts on a keyframe"
	);

	let asked_at = Instant::now();
	assert!(control.request_keyframe(full.ssrc), "mockcord sent a PLI");
	voice
		.voice_until("a keyframe after the PLI", |recording| {
			frames_after(recording, full.ssrc, asked_at)
				.iter()
				.any(VoiceVideoFrame::is_keyframe)
		})
		.await;

	let turned_off = voice.command(Command::SetSelfVideo(false));
	voice
		.op_4(turned_off, "op 4 with the camera off", |frame| {
			frame.data["self_video"] == json!(false)
		})
		.await;
	let recording = voice
		.voice_until("op 12 clearing the camera", |recording| {
			!video_states(recording, turned_off).is_empty()
		})
		.await;
	let cleared = video_states(&recording, turned_off)[0];
	assert_eq!(cleared.data["audio_ssrc"], json!(audio_ssrc));
	assert_eq!(cleared.data["video_ssrc"], json!(0));
	assert_eq!(cleared.data["rtx_ssrc"], json!(0));
	assert_eq!(cleared.data["streams"], json!([]));
	voice
		.running
		.until("the camera to show as off", |state| {
			state.voice().is_some_and(|session| !session.camera_on)
		})
		.await;
	tokio::time::sleep(DRAIN).await;
	let stopped_at = Instant::now();
	tokio::time::sleep(DRAIN).await;
	let recording = control.recording();
	assert!(
		frames_after(&recording, full.ssrc, stopped_at).is_empty()
			&& frames_after(&recording, half.ssrc, stopped_at).is_empty(),
		"no video after the camera is off"
	);
	voice.speaks(audio_ssrc, "audio after the camera").await;
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_an_h264_encoder_the_camera_says_video_unavailable_and_audio_goes_on() {
	let voice = start("camera-unavailable").await;
	voice.running.session().set_voice_setup(session::VoiceSetup {
		codecs: VideoCodecs::Missing,
		..manual_setup(&voice.running)
	});
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let audio_ssrc = voice.ssrc();
	let asked = voice.command(Command::SetSelfVideo(true));
	voice
		.running
		.until("the camera to fail with a toast", |state| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.reason.starts_with("Video unavailable"))
		})
		.await;
	assert!(!camera_on(&voice), "the camera stays off");
	let recording = voice.running.voice().recording();
	assert!(
		video_states(&recording, asked).is_empty(),
		"no op 12 without an encoder"
	);
	assert!(recording.video.is_empty(), "no video was sent");
	let self_video = voice
		.running
		.mock()
		.recording()
		.frames_with(protocol::Opcode::VoiceStateUpdate)
		.any(|frame| frame.at > asked && frame.data["self_video"] == json!(true));
	assert!(!self_video, "op 4 never claims the camera");
	voice.speaks(audio_ssrc, "audio keeps flowing").await;
	voice.stop().await;
}
