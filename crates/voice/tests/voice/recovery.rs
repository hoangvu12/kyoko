use std::time::Duration;

use voice::{StopReason, VoiceEvent};

use crate::harness::{CHANNEL_ID, Call, GUILD_ID, SESSION_ID};

const IDENTIFY: i64 = 0;
const RESUME: i64 = 7;
const SPEAKING: i64 = 5;
const REMOTE_USER: u64 = 222_222_222_222_222_222;
const REMOTE_SSRC: u32 = 9_000;
const QUIET_AFTER_STOP: Duration = Duration::from_millis(2_500);

async fn connected(call: &mut Call) -> u32 {
	match call
		.event("connected", |event| matches!(event, VoiceEvent::Connected { .. }))
		.await
	{
		VoiceEvent::Connected { ssrc, .. } => ssrc,
		_ => unreachable!(),
	}
}

async fn hear_remote_speaking(call: &mut Call) -> i64 {
	call.control().speak_as(REMOTE_USER, REMOTE_SSRC, true);
	call.event("the remote member speaking", |event| {
		matches!(event, VoiceEvent::Speaking { speaking: true, .. })
	})
	.await;
	call.recording()
		.sent_with(SPEAKING)
		.filter_map(|frame| frame.seq)
		.max()
		.expect("the speaking frame carried a seq")
}

async fn assert_resumes(call: &mut Call, seq: i64) {
	let recording = call
		.until("a resume", |recording| {
			recording.frames_with(RESUME).next().is_some()
		})
		.await;
	let resume = recording.frames_with(RESUME).next().expect("a resume");
	assert_eq!(
		resume.keys(&["d"]),
		["server_id", "channel_id", "session_id", "token", "seq_ack"]
	);
	assert_eq!(resume.data["server_id"], GUILD_ID.to_string());
	assert_eq!(resume.data["channel_id"], CHANNEL_ID.to_string());
	assert_eq!(resume.data["session_id"], SESSION_ID);
	assert_eq!(resume.data["seq_ack"], seq);
	call.event("resumed", |event| *event == VoiceEvent::Resumed).await;
	assert_eq!(call.recording().frames_with(IDENTIFY).count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4015_resumes_with_seq_ack_and_the_channel() {
	let mut call = Call::start(|_| {}).await;
	connected(&mut call).await;
	let seq = hear_remote_speaking(&mut call).await;
	call.control().close_sockets(4015);
	call.event("reconnecting by resume", |event| {
		*event
			== VoiceEvent::Reconnecting {
				code: Some(4015),
				resume: true,
			}
	})
	.await;
	assert_resumes(&mut call, seq).await;
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_network_resumes() {
	let mut call = Call::start(|_| {}).await;
	connected(&mut call).await;
	let seq = hear_remote_speaking(&mut call).await;
	call.control().drop_sockets();
	call.event("reconnecting by resume", |event| {
		*event
			== VoiceEvent::Reconnecting {
				code: None,
				resume: true,
			}
	})
	.await;
	assert_resumes(&mut call, seq).await;
	call.stop().await;
}

async fn starts_fresh_after(code: u16) {
	let mut call = Call::start(|_| {}).await;
	let first = connected(&mut call).await;
	call.control().close_sockets(code);
	call.event("reconnecting fresh", |event| {
		*event
			== VoiceEvent::Reconnecting {
				code: Some(code),
				resume: false,
			}
	})
	.await;
	let second = connected(&mut call).await;
	assert_ne!(first, second);
	let recording = call.recording();
	assert_eq!(recording.frames_with(IDENTIFY).count(), 2);
	assert_eq!(recording.frames_with(RESUME).count(), 0);
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4006_starts_a_fresh_session() {
	starts_fresh_after(4006).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4009_starts_a_fresh_session() {
	starts_fresh_after(4009).await;
}

async fn stops_after(code: u16, reason: StopReason) {
	let mut call = Call::start(|_| {}).await;
	connected(&mut call).await;
	call.control().close_sockets(code);
	let stopped = call
		.event("stopped", |event| matches!(event, VoiceEvent::Stopped(_)))
		.await;
	assert_eq!(stopped, VoiceEvent::Stopped(reason));
	assert_eq!(call.voice.ssrc(), None);
	tokio::time::sleep(QUIET_AFTER_STOP).await;
	let recording = call.recording();
	assert_eq!(recording.frames_with(IDENTIFY).count(), 1);
	assert_eq!(recording.frames_with(RESUME).count(), 0);
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4014_stops_as_disconnected() {
	stops_after(4014, StopReason::Disconnected).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4017_stops_because_e2ee_is_required() {
	stops_after(4017, StopReason::E2eeRequired).await;
}
