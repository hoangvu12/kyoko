use mockcord::{AES_GCM_MODE, VoiceRecording, XCHACHA_MODE};
use voice::{EncryptionMode, FRAME_SAMPLES, MixedFrame, SAMPLE_RATE, SILENCE_FRAME, VoiceEvent};

use crate::harness::Call;

const SELECT_PROTOCOL: i64 = 1;
const SPEAKING: i64 = 5;
const TONE_HZ: f32 = 1_000.0;
const TONE_AMPLITUDE: f32 = 0.25;
const TONE_FRAMES: usize = 50;
const QUIET_FRAMES: usize = 40;
const SILENCE_FRAMES: usize = 5;
const SETTLE_FRAMES: usize = 5;
const MEASURED_FRAMES: usize = 20;
const ECHO_SSRC_OFFSET: u32 = 1;
const FREQUENCY_TOLERANCE_HZ: f32 = 40.0;
const AUDIBLE_PEAK: i16 = 2_000;

fn tone_frame(index: usize) -> Vec<i16> {
	(0..FRAME_SAMPLES)
		.map(|sample| {
			let at = (index * FRAME_SAMPLES + sample) as f32 / SAMPLE_RATE as f32;
			let value = (at * TONE_HZ * std::f32::consts::TAU).sin() * TONE_AMPLITUDE;
			(value * f32::from(i16::MAX)) as i16
		})
		.collect()
}

fn feed_tone_then_quiet(call: &Call) {
	for index in 0..TONE_FRAMES {
		call.voice.feed_input(&tone_frame(index));
	}
	for _ in 0..QUIET_FRAMES {
		call.voice.feed_input(&[0; FRAME_SAMPLES]);
	}
}

fn left_channel(frames: &[MixedFrame]) -> Vec<f32> {
	frames
		.iter()
		.flat_map(|frame| frame.pcm.iter().step_by(2).map(|sample| f32::from(*sample)))
		.collect()
}

fn dominant_frequency(samples: &[f32]) -> f32 {
	let crossings = samples
		.windows(2)
		.filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
		.count();
	crossings as f32 / 2.0 / (samples.len() as f32 / SAMPLE_RATE as f32)
}

async fn echoed_tone(call: &Call, echo_ssrc: u32) -> Vec<MixedFrame> {
	let mut heard = Vec::new();
	let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
	while heard.len() < SETTLE_FRAMES + MEASURED_FRAMES {
		assert!(std::time::Instant::now() < deadline, "the tone never came back");
		heard.extend(call.voice.take_output().into_iter().filter(|frame| {
			frame.sources == [echo_ssrc] && frame.pcm.iter().any(|sample| sample.abs() > AUDIBLE_PEAK)
		}));
		tokio::time::sleep(std::time::Duration::from_millis(20)).await;
	}
	heard
}

async fn connected(call: &mut Call) -> (u32, EncryptionMode) {
	match call
		.event("connected", |event| matches!(event, VoiceEvent::Connected { .. }))
		.await
	{
		VoiceEvent::Connected { ssrc, mode } => (ssrc, mode),
		_ => unreachable!(),
	}
}

async fn round_trip(call: &mut Call, ssrc: u32, mode: &str) -> VoiceRecording {
	feed_tone_then_quiet(call);
	let heard = echoed_tone(call, ssrc + ECHO_SSRC_OFFSET).await;
	let frequency = dominant_frequency(&left_channel(&heard[SETTLE_FRAMES..]));
	assert!(
		(frequency - TONE_HZ).abs() < FREQUENCY_TOLERANCE_HZ,
		"the echo came back at {frequency} Hz"
	);
	let recording = call
		.until("the end of speech", |recording| {
			recording
				.frames_with(SPEAKING)
				.any(|frame| frame.data["speaking"] == 0 && frame.at > first_packet_at(recording, ssrc))
		})
		.await;
	assert!(recording.packets_from(ssrc).all(|packet| packet.mode == mode));
	recording
}

fn first_packet_at(recording: &VoiceRecording, ssrc: u32) -> std::time::Instant {
	recording
		.packets_from(ssrc)
		.map(|packet| packet.at)
		.min()
		.unwrap_or_else(|| std::time::Instant::now() + std::time::Duration::from_secs(3_600))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aes_gcm_is_chosen_when_offered_and_a_tone_comes_back_from_the_echo() {
	let mut call = Call::start(|_| {}).await;
	let (ssrc, mode) = connected(&mut call).await;
	assert_eq!(mode, EncryptionMode::AeadAes256GcmRtpSize);
	let recording = round_trip(&mut call, ssrc, AES_GCM_MODE).await;
	let select = recording
		.frames_with(SELECT_PROTOCOL)
		.next()
		.expect("a select protocol");
	assert_eq!(select.data["data"]["mode"], AES_GCM_MODE);
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xchacha_is_chosen_when_it_is_all_the_server_offers() {
	let mut call = Call::start(|control| control.set_modes(&[XCHACHA_MODE])).await;
	let (ssrc, mode) = connected(&mut call).await;
	assert_eq!(mode, EncryptionMode::AeadXChaCha20Poly1305RtpSize);
	let recording = round_trip(&mut call, ssrc, XCHACHA_MODE).await;
	let select = recording
		.frames_with(SELECT_PROTOCOL)
		.next()
		.expect("a select protocol");
	assert_eq!(select.data["data"]["mode"], XCHACHA_MODE);
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn speech_ends_with_five_silence_frames_and_op_5_comes_before_media() {
	let mut call = Call::start(|_| {}).await;
	let (ssrc, _) = connected(&mut call).await;
	feed_tone_then_quiet(&call);
	let recording = call
		.until("speaking to stop after the silence frames", |recording| {
			let silences = recording
				.packets_from(ssrc)
				.filter(|packet| packet.payload == SILENCE_FRAME)
				.count();
			silences >= SILENCE_FRAMES
				&& recording
					.frames_with(SPEAKING)
					.any(|frame| frame.data["speaking"] == 0 && frame.at > first_packet_at(recording, ssrc))
		})
		.await;
	let speaking: Vec<_> = recording.frames_with(SPEAKING).collect();
	assert_eq!(speaking[0].data["speaking"], 0);
	assert_eq!(speaking[0].data["ssrc"], ssrc);
	assert!(speaking[0].at < first_packet_at(&recording, ssrc));
	assert_eq!(
		speaking
			.iter()
			.map(|frame| frame.data["speaking"].as_u64())
			.collect::<Vec<_>>(),
		[Some(0), Some(1), Some(0)]
	);
	let payloads: Vec<&[u8]> = recording
		.packets_from(ssrc)
		.map(|packet| packet.payload.as_slice())
		.collect();
	let silences = payloads
		.iter()
		.filter(|payload| **payload == SILENCE_FRAME)
		.count();
	assert_eq!(silences, SILENCE_FRAMES);
	let tail = &payloads[payloads.len() - SILENCE_FRAMES..];
	assert!(tail.iter().all(|payload| *payload == SILENCE_FRAME));
	assert_ne!(payloads[payloads.len() - SILENCE_FRAMES - 1], SILENCE_FRAME);
	call.stop().await;
}
