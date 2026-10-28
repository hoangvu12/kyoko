use std::time::{Duration, Instant};

use mockcord::{VoiceControl, VoicePacket, VoiceRecording};
use opus::{Application, Channels, Encoder};
use protocol::Snowflake;
use voice::{FRAME_SAMPLES, SILENCE_FRAME, VoiceEvent};

use crate::harness::{CHANNEL_ID, Call, USER_ID};
use crate::media::tone_frame;

const MEMBER: u64 = 222_801_430_442_967_040;
const MEMBER_SSRC: u32 = 9_000;
const NEWCOMER: u64 = 333_014_820_093_313_024;
const KEY_PACKAGE: i64 = 26;
const TRANSITION_READY: i64 = 23;
const INVALID_COMMIT_WELCOME: i64 = 31;
const EXTERNAL_SENDER: i64 = 25;
const WELCOME: i64 = 30;
const MARKER: [u8; 2] = [0xfa, 0xfa];
const LONG_SPEECH_FRAMES: usize = 1_500;
const SHORT_SPEECH_FRAMES: usize = 50;
const QUIET_FRAMES: usize = 40;
const SILENCE_FRAMES: usize = 5;
const ENOUGH_PACKETS: usize = 15;
const MEMBER_FRAMES: usize = 60;
const AUDIBLE_PEAK: i16 = 2_000;
const HEARD_FRAMES: usize = 10;
const FRAME: Duration = Duration::from_millis(20);
const PATIENCE: Duration = Duration::from_secs(20);

fn with_member(control: &VoiceControl) {
	control.set_dave_version(1);
	control.add_member(CHANNEL_ID.get(), MEMBER);
}

async fn epoch(call: &mut Call, epoch: u64, version: u8) -> Vec<Snowflake> {
	let event = call
		.event("a DAVE epoch", |event| {
			matches!(event, VoiceEvent::Dave { epoch: seen, version: shown, .. } if *seen == epoch && *shown == version)
		})
		.await;
	match event {
		VoiceEvent::Dave { members, .. } => members,
		_ => unreachable!(),
	}
}

async fn ssrc(call: &mut Call) -> u32 {
	match call
		.event("connected", |event| matches!(event, VoiceEvent::Connected { .. }))
		.await
	{
		VoiceEvent::Connected { ssrc, .. } => ssrc,
		_ => unreachable!(),
	}
}

fn feed(call: &Call, frames: usize) {
	for index in 0..frames {
		call.voice.feed_input(&tone_frame(index));
	}
}

fn sealed(packet: &VoicePacket) -> bool {
	packet.payload.ends_with(&MARKER)
}

fn plain_speech(packet: &VoicePacket) -> bool {
	!sealed(packet) && packet.payload != SILENCE_FRAME
}

async fn packets_after(
	call: &Call,
	what: &str,
	ssrc: u32,
	since: Instant,
	wanted: fn(&VoicePacket) -> bool,
) -> VoiceRecording {
	call.until(what, |recording| {
		recording
			.packets_from(ssrc)
			.filter(|packet| packet.at > since && wanted(packet))
			.count() >= ENOUGH_PACKETS
	})
	.await
}

fn every_sealed_packet_opens(recording: &VoiceRecording, ssrc: u32) {
	let sealed: Vec<&VoicePacket> = recording
		.packets_from(ssrc)
		.filter(|packet| sealed(packet))
		.collect();
	assert!(!sealed.is_empty(), "kyoko sent DAVE frames");
	for packet in sealed {
		let opened = packet
			.opened
			.as_ref()
			.expect("every DAVE frame kyoko sent decrypts for a member");
		assert_ne!(opened, &packet.payload);
		assert!(!opened.ends_with(&MARKER));
	}
}

async fn kyoko_hears(call: &mut Call, member: u64, ssrc: u32) {
	let control = call.control();
	control.speak_as(member, ssrc, true);
	let mut encoder = Encoder::new(48_000, Channels::Mono, Application::Voip).expect("an opus encoder");
	let mut packet = vec![0; 1_275];
	let mut heard = 0;
	let deadline = Instant::now() + PATIENCE;
	let mut sequence: u16 = 0;
	while heard < HEARD_FRAMES {
		assert!(Instant::now() < deadline, "kyoko never heard the member");
		for index in 0..MEMBER_FRAMES {
			let length = encoder
				.encode(&tone_frame(index), &mut packet)
				.expect("the tone encodes");
			sequence = sequence.wrapping_add(1);
			control.send_as(CHANNEL_ID.get(), member, ssrc, sequence, &packet[..length]);
			tokio::time::sleep(FRAME).await;
			heard += call
				.voice
				.take_output()
				.into_iter()
				.filter(|frame| {
					frame.sources == [ssrc] && frame.pcm.iter().any(|sample| sample.abs() > AUDIBLE_PEAK)
				})
				.count();
		}
	}
	control.speak_as(member, ssrc, false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kyoko_joins_the_dave_group_and_its_opus_frames_decrypt_for_the_member() {
	let mut call = Call::start(with_member).await;
	let ssrc = ssrc(&mut call).await;
	let members = epoch(&mut call, 1, 1).await;
	assert_eq!(members.len(), 2);
	assert!(members.contains(&USER_ID) && members.contains(&Snowflake(MEMBER)));
	assert!(call.voice.dave().enabled());
	feed(&call, SHORT_SPEECH_FRAMES);
	for _ in 0..QUIET_FRAMES {
		call.voice.feed_input(&[0; FRAME_SAMPLES]);
	}
	let recording = call
		.until("the end of speech in silence frames", |recording| {
			recording
				.packets_from(ssrc)
				.filter(|packet| packet.payload == SILENCE_FRAME)
				.count() >= SILENCE_FRAMES
		})
		.await;
	every_sealed_packet_opens(&recording, ssrc);
	assert!(
		recording
			.packets_from(ssrc)
			.filter(|packet| sealed(packet))
			.count() >= ENOUGH_PACKETS
	);
	let silences: Vec<&VoicePacket> = recording
		.packets_from(ssrc)
		.filter(|packet| packet.payload == SILENCE_FRAME)
		.collect();
	assert!(
		silences
			.iter()
			.all(|packet| packet.opened.as_deref() == Some(&SILENCE_FRAME[..]))
	);
	let key_package = recording
		.frames
		.iter()
		.find(|frame| frame.is_binary() && frame.op == KEY_PACKAGE)
		.expect("kyoko sends its key package");
	assert!(!key_package.payload.is_empty());
	let welcome = recording
		.sent
		.iter()
		.find(|frame| frame.is_binary() && frame.op == WELCOME)
		.expect("kyoko is welcomed");
	assert!(welcome.seq.is_some());
	assert!(
		recording
			.sent
			.iter()
			.any(|frame| frame.is_binary() && frame.op == EXTERNAL_SENDER)
	);
	let transition = u16::from_be_bytes([welcome.payload[0], welcome.payload[1]]);
	assert!(
		recording
			.frames_with(TRANSITION_READY)
			.any(|frame| frame.data["transition_id"] == transition)
	);
	kyoko_hears(&mut call, MEMBER, MEMBER_SSRC).await;
	assert!(call.voice.dave().decrypted() > 0);
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn members_joining_and_leaving_move_the_epoch_while_media_keeps_flowing() {
	let mut call = Call::start(with_member).await;
	let ssrc = ssrc(&mut call).await;
	epoch(&mut call, 1, 1).await;
	feed(&call, LONG_SPEECH_FRAMES);
	packets_after(&call, "speech in epoch 1", ssrc, Instant::now(), sealed).await;
	call.control().add_member(CHANNEL_ID.get(), NEWCOMER);
	let members = epoch(&mut call, 2, 1).await;
	assert_eq!(members.len(), 3);
	assert_eq!(call.control().member_epoch(CHANNEL_ID.get(), NEWCOMER), Some(2));
	packets_after(&call, "speech in epoch 2", ssrc, Instant::now(), sealed).await;
	call.control().remove_member(CHANNEL_ID.get(), NEWCOMER);
	let members = epoch(&mut call, 3, 1).await;
	assert_eq!(members.len(), 2);
	assert!(!members.contains(&Snowflake(NEWCOMER)));
	let recording = packets_after(&call, "speech in epoch 3", ssrc, Instant::now(), sealed).await;
	every_sealed_packet_opens(&recording, ssrc);
	let gaps = recording
		.packets_from(ssrc)
		.collect::<Vec<_>>()
		.windows(2)
		.map(|pair| pair[1].at.duration_since(pair[0].at))
		.max()
		.unwrap_or_default();
	assert!(gaps < Duration::from_millis(500), "media stalled for {gaps:?}");
	kyoko_hears(&mut call, MEMBER, MEMBER_SSRC).await;
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invalid_commit_sends_op_31_and_kyoko_is_welcomed_back() {
	let mut call = Call::start(with_member).await;
	let ssrc = ssrc(&mut call).await;
	epoch(&mut call, 1, 1).await;
	call.control().send_bogus_commit(CHANNEL_ID.get(), USER_ID.get());
	call.event("a DAVE failure", |event| {
		matches!(event, VoiceEvent::DaveFailure(_))
	})
	.await;
	let members = epoch(&mut call, 3, 1).await;
	assert_eq!(members.len(), 2);
	let recording = call.recording();
	assert_eq!(recording.frames_with(INVALID_COMMIT_WELCOME).count(), 1);
	assert_eq!(
		recording
			.frames
			.iter()
			.filter(|frame| frame.is_binary() && frame.op == KEY_PACKAGE)
			.count(),
		2
	);
	let since = Instant::now();
	feed(&call, SHORT_SPEECH_FRAMES * 2);
	let recording = packets_after(&call, "speech after recovering", ssrc, since, sealed).await;
	every_sealed_packet_opens(&recording, ssrc);
	kyoko_hears(&mut call, MEMBER, MEMBER_SSRC).await;
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_downgrade_sends_plain_opus_and_an_upgrade_encrypts_again() {
	let mut call = Call::start(with_member).await;
	let ssrc = ssrc(&mut call).await;
	epoch(&mut call, 1, 1).await;
	feed(&call, LONG_SPEECH_FRAMES);
	packets_after(&call, "encrypted speech", ssrc, Instant::now(), sealed).await;
	call.control().downgrade_dave(CHANNEL_ID.get());
	epoch(&mut call, 1, 0).await;
	assert!(!call.voice.dave().enabled());
	let recording = packets_after(
		&call,
		"plain speech after the downgrade",
		ssrc,
		Instant::now(),
		plain_speech,
	)
	.await;
	let latest = recording.packets_from(ssrc).last().expect("kyoko keeps sending");
	assert!(!sealed(latest));
	call.control().upgrade_dave(CHANNEL_ID.get());
	epoch(&mut call, 1, 1).await;
	assert!(call.voice.dave().enabled());
	let recording = packets_after(
		&call,
		"encrypted speech after the upgrade",
		ssrc,
		Instant::now(),
		sealed,
	)
	.await;
	let latest = recording.packets_from(ssrc).last().expect("kyoko keeps sending");
	assert!(sealed(latest));
	every_sealed_packet_opens(&recording, ssrc);
	kyoko_hears(&mut call, MEMBER, MEMBER_SSRC).await;
	call.stop().await;
}
