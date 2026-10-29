use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use denoise::testing::{level_db, speech};
use mockcord::VoiceControl;
use opus::{Application, Channels, Decoder, Encoder};
use session::Command;
use store::VoiceSettings;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use voice::{FRAME_SAMPLES, MixedFrame, SILENCE_FRAME, VoiceConnection};

use crate::voice::{SETTLE, Voice, start};

const FAR_END_USER: u64 = 222_801_430_442_967_040;
const FAR_END_SSRC: u32 = 9_100;
const OPUS_PACKET_BYTES: usize = 1_275;
const FRAME: Duration = Duration::from_millis(20);
const POLL: Duration = Duration::from_millis(5);
const MARKER_FRAMES: usize = 5;
const MARKER_AMPLITUDE: f32 = 0.1;
const MARKER_HZ: f32 = 440.0;
const PATH_FRAMES: usize = 3;
const PATH_SAMPLES: usize = 240;
const ECHO_GAIN: f32 = 0.5;
const ECHO_FRAMES: usize = 500;
const DOUBLE_TALK_FRAMES: usize = 200;
const CONVERGED: usize = 200;
const SECOND_FRAMES: usize = 50;
const DOUBLE_TALK_SETTLED: usize = 20;
const NEAR_END_GAIN: f32 = 0.5;
const ERLE_ON_DB: f32 = 20.0;
const ERLE_OFF_DB: f32 = 3.0;
const NEAR_END_KEPT_DB: f32 = 6.0;
const QUIET_DB: f32 = -45.0;
const LOUD_DB: f32 = -15.0;
const SPEAKER_FRAMES: usize = 600;
const SPEAKER_SETTLED: usize = 400;
const LEVELLED: std::ops::RangeInclusive<f32> = -30.0..=-10.0;
const PASS_THROUGH_DB: f32 = 3.0;
const ROUND_TRIP_SLACK: Duration = Duration::from_millis(800);

fn connection(voice: &Voice) -> Arc<VoiceConnection> {
	voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection")
}

fn pcm(samples: &[f32]) -> Vec<i16> {
	samples
		.iter()
		.map(|sample| (sample * f32::from(i16::MAX)).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16)
		.collect()
}

fn mono(frame: &MixedFrame) -> Vec<f32> {
	frame
		.pcm
		.as_chunks::<2>()
		.0
		.iter()
		.map(|[left, right]| (f32::from(*left) + f32::from(*right)) * 0.5 / f32::from(i16::MAX))
		.collect()
}

fn looped(samples: &[f32], length: usize) -> Vec<f32> {
	samples.iter().copied().cycle().take(length).collect()
}

fn at_level(samples: &[f32], level: f32) -> Vec<f32> {
	let gain = 10f32.powf((level - level_db(samples)) / 20.0);
	samples.iter().map(|sample| sample * gain).collect()
}

fn marker() -> Vec<f32> {
	(0..MARKER_FRAMES * FRAME_SAMPLES)
		.map(|index| {
			let at = index as f32 / voice::SAMPLE_RATE as f32;
			(at * MARKER_HZ * std::f32::consts::TAU).sin() * MARKER_AMPLITUDE
		})
		.collect()
}

fn span(frames: std::ops::Range<usize>) -> std::ops::Range<usize> {
	frames.start * FRAME_SAMPLES..frames.end * FRAME_SAMPLES
}

struct Sent {
	seen: usize,
}

impl Sent {
	async fn marked(voice: &Voice) -> Self {
		let seen = voice
			.running
			.voice()
			.recording()
			.packets_from(voice.ssrc())
			.count();
		let connection = connection(voice);
		for frame in marker().as_chunks::<FRAME_SAMPLES>().0 {
			connection.feed_input(&pcm(frame));
		}
		tokio::time::sleep(FRAME * MARKER_FRAMES as u32 + SETTLE).await;
		let _ = connection.take_output();
		Self { seen }
	}

	fn received(&self, voice: &Voice) -> Vec<f32> {
		let recording = voice.running.voice().recording();
		let packets: Vec<_> = recording
			.packets_from(voice.ssrc())
			.skip(self.seen)
			.filter(|packet| packet.payload != SILENCE_FRAME)
			.collect();
		let first = packets.first().expect("the marker was sent").timestamp;
		let mut decoder = Decoder::new(48_000, Channels::Mono).expect("an opus decoder");
		let mut output = Vec::new();
		let mut frame = vec![0.0f32; FRAME_SAMPLES];
		for packet in packets {
			let at = packet.timestamp.wrapping_sub(first) as usize;
			let decoded = decoder
				.decode_float(&packet.payload, &mut frame, false)
				.expect("the sent opus decodes");
			if output.len() < at + decoded {
				output.resize(at + decoded, 0.0);
			}
			output[at..at + decoded].copy_from_slice(&frame[..decoded]);
		}
		output.drain(..(MARKER_FRAMES * FRAME_SAMPLES).min(output.len()));
		output
	}
}

fn play_far_end(control: VoiceControl, channel_id: u64, far_end: Vec<f32>) -> JoinHandle<()> {
	control.speak_as(FAR_END_USER, FAR_END_SSRC, true);
	tokio::spawn(async move {
		let mut encoder = Encoder::new(48_000, Channels::Mono, Application::Voip).expect("an opus encoder");
		let mut packet = vec![0; OPUS_PACKET_BYTES];
		let mut ticker = interval(FRAME);
		ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);
		for (index, frame) in far_end.as_chunks::<FRAME_SAMPLES>().0.iter().enumerate() {
			ticker.tick().await;
			let length = encoder
				.encode_float(frame, &mut packet)
				.expect("the far end encodes");
			control.send_as(
				channel_id,
				FAR_END_USER,
				FAR_END_SSRC,
				index as u16,
				&packet[..length],
			);
		}
	})
}

struct Microphone {
	echo: Vec<f32>,
	near_end: Vec<f32>,
}

async fn echo_path(connection: Arc<VoiceConnection>, near_end: Vec<f32>, frames: usize) -> Microphone {
	let mut path: VecDeque<f32> = std::iter::repeat_n(0.0, PATH_SAMPLES).collect();
	let mut echo = Vec::with_capacity(frames * FRAME_SAMPLES);
	let _ = connection.take_output();
	for _ in 0..PATH_FRAMES {
		connection.feed_input(&[0; FRAME_SAMPLES]);
		echo.extend([0.0; FRAME_SAMPLES]);
	}
	while echo.len() < frames * FRAME_SAMPLES {
		for played in connection.take_output() {
			path.extend(mono(&played));
			let start = echo.len();
			echo.extend(path.drain(..FRAME_SAMPLES).map(|sample| sample * ECHO_GAIN));
			let heard: Vec<f32> = (start..echo.len())
				.map(|index| echo[index] + near_end.get(index).copied().unwrap_or(0.0))
				.collect();
			connection.feed_input(&pcm(&heard));
			if echo.len() >= frames * FRAME_SAMPLES {
				break;
			}
		}
		tokio::time::sleep(POLL).await;
	}
	Microphone { echo, near_end }
}

async fn processing(name: &'static str, change: impl FnOnce(&mut VoiceSettings)) -> Voice {
	let voice = start(name).await;
	voice
		.running
		.store()
		.update_settings(|settings| {
			settings.voice.push_to_talk = true;
			change(&mut settings.voice);
		})
		.expect("the settings save");
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	voice.command(Command::SetPushToTalk(true));
	voice
}

struct EchoRun {
	microphone: Microphone,
	sent: Vec<f32>,
}

impl EchoRun {
	async fn record(voice: &Voice, near_end: Vec<f32>, frames: usize) -> Self {
		let control = voice.running.voice();
		let sent = Sent::marked(voice).await;
		let far_end = looped(&speech(), (frames + PATH_FRAMES) * FRAME_SAMPLES);
		let player = play_far_end(control.clone(), voice.lounge.get(), far_end);
		let microphone = echo_path(connection(voice), near_end, frames).await;
		player.abort();
		tokio::time::sleep(ROUND_TRIP_SLACK).await;
		Self {
			microphone,
			sent: sent.received(voice),
		}
	}

	fn sent_in(&self, frames: std::ops::Range<usize>) -> &[f32] {
		let span = span(frames);
		&self.sent[span.start.min(self.sent.len())..span.end.min(self.sent.len())]
	}

	fn median_erle_db(&self, frames: std::ops::Range<usize>) -> f32 {
		let mut seconds: Vec<f32> = frames
			.step_by(SECOND_FRAMES)
			.map(|start| self.erle_db(start..start + SECOND_FRAMES))
			.collect();
		seconds.sort_by(f32::total_cmp);
		seconds[seconds.len() / 2]
	}

	fn erle_db(&self, frames: std::ops::Range<usize>) -> f32 {
		let echo = level_db(&self.microphone.echo[span(frames.clone())]);
		let residual = self.sent_in(frames.clone());
		let energy: f32 = residual.iter().map(|sample| sample * sample).sum();
		let residual_db = 10.0 * (energy / (span(frames).len() as f32) + 1e-12).log10();
		echo - residual_db
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn echo_cancellation_removes_the_calls_echo_keeps_your_voice_and_turns_off_live() {
	assert!(
		VoiceSettings::default().echo_cancellation,
		"echo cancellation is on by default"
	);
	let voice = processing("voice-echo-cancellation", |settings| {
		settings.echo_cancellation = true;
	})
	.await;
	voice.running.voice().hold_echo();
	let connection = connection(&voice);
	let near_end: Vec<f32> = std::iter::repeat_n(0.0, ECHO_FRAMES * FRAME_SAMPLES)
		.chain(
			looped(&speech(), DOUBLE_TALK_FRAMES * FRAME_SAMPLES)
				.into_iter()
				.map(|sample| sample * NEAR_END_GAIN),
		)
		.collect();
	let on = EchoRun::record(&voice, near_end, ECHO_FRAMES + DOUBLE_TALK_FRAMES).await;
	let erle_on = on.median_erle_db(CONVERGED..ECHO_FRAMES);
	let talk = ECHO_FRAMES + DOUBLE_TALK_SETTLED..ECHO_FRAMES + DOUBLE_TALK_FRAMES;
	let near_in = level_db(&on.microphone.near_end[span(talk.clone())]);
	let near_out = level_db(on.sent_in(talk));
	eprintln!(
		"echo cancellation on: ERLE {erle_on:.1} dB, near end {near_in:.1} -> {near_out:.1} dB, {:?}",
		connection.stats()
	);
	assert!(
		erle_on >= ERLE_ON_DB,
		"echo cancellation takes the echo down by {ERLE_ON_DB} dB: {erle_on:.1} dB"
	);
	assert!(
		(near_out - near_in).abs() <= NEAR_END_KEPT_DB,
		"your voice over the far end stays within {NEAR_END_KEPT_DB} dB: {near_in:.1} -> {near_out:.1} dB"
	);
	assert!(connection.stats().frames_echo_cancelled > 0);

	voice.change_voice_settings(|settings| settings.echo_cancellation = false);
	tokio::time::sleep(SETTLE).await;
	let cancelled = connection.stats().frames_echo_cancelled;
	let off = EchoRun::record(&voice, Vec::new(), ECHO_FRAMES).await;
	let erle_off = off.median_erle_db(CONVERGED..ECHO_FRAMES);
	eprintln!("echo cancellation off: ERLE {erle_off:.1} dB");
	assert!(
		erle_off.abs() <= ERLE_OFF_DB,
		"without echo cancellation the echo is sent as it came: {erle_off:.1} dB"
	);
	assert_eq!(
		connection.stats().frames_echo_cancelled,
		cancelled,
		"no frame goes through the echo canceller once it is off"
	);
	voice.stop().await;
}

async fn spoken_at(voice: &Voice, level: f32) -> f32 {
	let sent = Sent::marked(voice).await;
	let speaker = at_level(&looped(&speech(), SPEAKER_FRAMES * FRAME_SAMPLES), level);
	let connection = connection(voice);
	for frame in speaker.as_chunks::<FRAME_SAMPLES>().0 {
		connection.feed_input(&pcm(frame));
	}
	tokio::time::sleep(FRAME * SPEAKER_FRAMES as u32 + ROUND_TRIP_SLACK).await;
	let received = sent.received(voice);
	let settled = span(SPEAKER_SETTLED..SPEAKER_FRAMES);
	let input = level_db(&speaker[settled.clone()]);
	let output = level_db(&received[settled.start.min(received.len())..settled.end.min(received.len())]);
	eprintln!("speaker at {input:.1} dB is sent at {output:.1} dB");
	output
}

#[tokio::test(flavor = "multi_thread")]
async fn automatic_gain_control_brings_a_quiet_and_a_loud_speaker_together_and_turns_off_live() {
	assert!(
		VoiceSettings::default().auto_gain_control,
		"automatic gain control is on by default"
	);
	let voice = processing("voice-auto-gain", |settings| {
		settings.auto_gain_control = true;
	})
	.await;
	let connection = connection(&voice);
	let quiet = spoken_at(&voice, QUIET_DB).await;
	let loud = spoken_at(&voice, LOUD_DB).await;
	assert!(
		LEVELLED.contains(&quiet) && LEVELLED.contains(&loud),
		"a quiet ({QUIET_DB} dB) and a loud ({LOUD_DB} dB) speaker are both sent within {LEVELLED:?} dB: {quiet:.1} and {loud:.1} dB"
	);
	assert!(connection.stats().frames_gain_controlled > 0);

	voice.change_voice_settings(|settings| settings.auto_gain_control = false);
	tokio::time::sleep(SETTLE).await;
	let controlled = connection.stats().frames_gain_controlled;
	let quiet_off = spoken_at(&voice, QUIET_DB).await;
	assert!(
		(quiet_off - QUIET_DB).abs() <= PASS_THROUGH_DB,
		"without gain control the quiet speaker is sent as it is: {quiet_off:.1} dB"
	);
	assert_eq!(
		connection.stats().frames_gain_controlled,
		controlled,
		"no frame goes through gain control once it is off"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_mic_test_plays_your_voice_back_through_gain_control() {
	let voice = start("voice-mic-test-processing").await;
	voice.change_voice_settings(|settings| {
		settings.echo_cancellation = true;
		settings.auto_gain_control = true;
	});
	voice.command(Command::StartMicTest);
	voice
		.running
		.eventually("the mic check to open", || {
			voice.running.session().mic_check().is_some()
		})
		.await;
	let check = voice.running.session().mic_check().expect("a running mic check");
	let speaker = at_level(&looped(&speech(), SPEAKER_FRAMES * FRAME_SAMPLES), QUIET_DB);
	let played = std::cell::RefCell::new(Vec::new());
	for frame in speaker.as_chunks::<FRAME_SAMPLES>().0 {
		check.feed_input(&pcm(frame));
		played.borrow_mut().extend(check.take_output());
		tokio::time::sleep(POLL).await;
	}
	voice
		.running
		.eventually("the voice to be played back", || {
			let mut played = played.borrow_mut();
			played.extend(check.take_output());
			played.len() >= SPEAKER_FRAMES
		})
		.await;
	let heard: Vec<f32> = played
		.into_inner()
		.iter()
		.flat_map(|frame| {
			frame
				.as_chunks::<2>()
				.0
				.iter()
				.map(|[left, right]| (f32::from(*left) + f32::from(*right)) * 0.5 / f32::from(i16::MAX))
				.collect::<Vec<f32>>()
		})
		.collect();
	let settled = span(SPEAKER_SETTLED..SPEAKER_FRAMES);
	let input = level_db(&speaker[settled.clone()]);
	let output = level_db(&heard[settled]);
	eprintln!("mic test: {input:.1} dB is played back at {output:.1} dB");
	assert!(
		LEVELLED.contains(&output),
		"the mic test raises a quiet voice into {LEVELLED:?} dB: {input:.1} -> {output:.1} dB"
	);
	voice.command(Command::StopMicTest);
	voice.stop().await;
}
