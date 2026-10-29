use std::sync::Arc;
use std::time::Duration;

use denoise::testing::{Clip, aligned, level_db, si_sdr_db};
use opus::{Channels, Decoder};
use protocol::Opcode;
use session::Command;
use state::{State, VoiceStatus};
use store::{NoiseSuppression, VoiceSettings};
use voice::{AudioDevicesInUse, FRAME_SAMPLES, SILENCE_FRAME, Suppression};

use crate::voice::{QUIET_WINDOW, SETTLE, TONE_FRAMES, Voice, identifies, manual_audio, start, tone_frame};

const QUIET_AMPLITUDE: f32 = 0.01;
const STRICT_SENSITIVITY_DB: i8 = -25;
const LOOSE_SENSITIVITY_DB: i8 = -60;
const SECOND_MIC: &str = "mock-microphone-2";
const SECOND_SPEAKERS: &str = "mock-speakers-2";
const MIC_TEST_SPEECH_DB: f32 = -30.0;
const MIC_TEST_QUIET_DB: f32 = -70.0;
const LOUDER_INPUT: u8 = 150;
const RELEASE_DELAY_MS: u16 = 1500;
const SNRS_DB: [f32; 3] = [0.0, 5.0, 10.0];
const HIGH_QUALITY_MARGINS_DB: [f32; 3] = [3.0, 1.5, 0.0];
const LOW_CPU_MARGINS_DB: [f32; 3] = [1.5, 0.0, -2.0];
const NOISE_DROP_DB: f32 = 20.0;
const SPEECH_KEPT_DB: f32 = 3.0;
const PASS_THROUGH_DB: f32 = 3.0;
const LEAD_IN: usize = 48_000;
const CLIP_SEED: u64 = 11;
const WARM_UP_SEED: u64 = 5;
const FRAME_TIME: Duration = Duration::from_millis(20);
const CLIP_SETTLE: Duration = Duration::from_millis(800);

#[tokio::test(flavor = "multi_thread")]
async fn a_device_change_while_connected_restarts_audio_without_reconnecting() {
	let voice = start("voice-device-change").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	voice.speaks(ssrc, "speech before the device change").await;
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	let restarts = connection.audio_restarts();

	voice.change_voice_settings(|settings| {
		settings.input_device = Some(SECOND_MIC.to_owned());
		settings.output_device = Some(SECOND_SPEAKERS.to_owned());
	});
	voice
		.running
		.eventually("both devices to restart", || {
			connection.audio_restarts() == restarts + 2
		})
		.await;
	assert_eq!(
		connection.audio_devices(),
		AudioDevicesInUse {
			input: Some(SECOND_MIC.to_owned()),
			output: Some(SECOND_SPEAKERS.to_owned()),
		}
	);
	voice.speaks(ssrc, "speech after the device change").await;
	let kept = voice
		.running
		.session()
		.voice_connection()
		.expect("still connected");
	assert!(Arc::ptr_eq(&kept, &connection), "the connection is kept");
	assert_eq!(
		identifies(&voice.running.voice().recording()).len(),
		1,
		"no second voice identify"
	);
	assert_eq!(
		voice
			.running
			.session()
			.state()
			.read()
			.voice()
			.map(|voice| voice.status.clone()),
		Some(VoiceStatus::Connected)
	);

	voice.change_voice_settings(|settings| settings.input_volume = LOUDER_INPUT);
	tokio::time::sleep(SETTLE).await;
	assert_eq!(
		connection.audio_restarts(),
		restarts + 2,
		"other settings leave the devices alone"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_input_sensitivity_threshold_gates_sending() {
	let voice = start("voice-sensitivity").await;
	voice
		.running
		.store()
		.update_settings(|settings| {
			settings.voice.auto_sensitivity = false;
			settings.voice.sensitivity_db = STRICT_SENSITIVITY_DB;
		})
		.expect("the settings save");
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	voice
		.stays_silent_at(
			ssrc,
			QUIET_AMPLITUDE,
			"a quiet voice stays under a strict threshold",
		)
		.await;
	voice.speaks(ssrc, "a loud voice passes a strict threshold").await;

	voice.change_voice_settings(|settings| settings.sensitivity_db = LOOSE_SENSITIVITY_DB);
	voice
		.speaks_at(ssrc, QUIET_AMPLITUDE, "a quiet voice passes a loose threshold")
		.await;

	voice.change_voice_settings(|settings| {
		settings.sensitivity_db = STRICT_SENSITIVITY_DB;
		settings.auto_sensitivity = true;
	});
	voice
		.speaks_at(
			ssrc,
			QUIET_AMPLITUDE,
			"automatic sensitivity ignores the manual threshold",
		)
		.await;
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn push_to_talk_sends_only_while_held_and_survives_a_restart() {
	let mut voice = start("voice-push-to-talk").await;
	voice
		.running
		.store()
		.update_settings(|settings| settings.voice.push_to_talk = true)
		.expect("the settings save");
	voice.running.restart().await;
	manual_audio(&voice.running);
	assert!(
		voice.running.store().settings.read().voice.push_to_talk,
		"push to talk is still chosen after a restart"
	);
	voice
		.running
		.until("the guild after the restart", |state| {
			state.guild(voice.guild_id).is_some()
		})
		.await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	voice
		.stays_silent(ssrc, "nothing is sent until the key is held")
		.await;

	voice.command(Command::SetPushToTalk(true));
	voice.speaks(ssrc, "speech while the key is held").await;

	voice.command(Command::SetPushToTalk(false));
	voice
		.stays_silent(ssrc, "nothing is sent once the key is released")
		.await;

	voice.change_voice_settings(|settings| settings.push_to_talk = false);
	voice
		.speaks(ssrc, "voice activity sends again without the key")
		.await;
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_release_delay_holds_the_mic_open_and_a_press_inside_it_keeps_it_open() {
	let voice = start("voice-release-delay").await;
	voice
		.running
		.store()
		.update_settings(|settings| {
			settings.voice.push_to_talk = true;
			settings.voice.push_to_talk_release_delay = RELEASE_DELAY_MS;
		})
		.expect("the settings save");
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	let delay = Duration::from_millis(u64::from(RELEASE_DELAY_MS));

	voice.command(Command::SetPushToTalk(true));
	voice.speaks(ssrc, "speech while the key is held").await;
	let released = voice.command(Command::SetPushToTalk(false));
	voice.speaks(ssrc, "speech inside the release delay").await;
	assert!(
		released.elapsed() < delay,
		"the speech was heard before the delay ran out"
	);
	voice
		.running
		.eventually("the mic to close after the delay", || !connection.transmitting())
		.await;
	assert!(released.elapsed() >= delay, "the mic closed only after the delay");
	voice
		.stays_silent(ssrc, "nothing is sent once the delay is over")
		.await;

	voice.command(Command::SetPushToTalk(true));
	voice.command(Command::SetPushToTalk(false));
	tokio::time::sleep(SETTLE).await;
	voice.command(Command::SetPushToTalk(true));
	tokio::time::sleep(delay).await;
	assert!(
		connection.transmitting(),
		"a press inside the delay keeps the mic open"
	);
	voice
		.speaks(ssrc, "speech after the first delay would have ended")
		.await;

	voice.command(Command::SetPushToTalk(false));
	voice
		.running
		.eventually("the mic to close after the last release", || {
			!connection.transmitting()
		})
		.await;
	voice.stop().await;
}

struct Measured {
	si_sdr_before_db: f32,
	si_sdr_after_db: f32,
	noise_before_db: f32,
	noise_after_db: f32,
	speech_before_db: f32,
	speech_after_db: f32,
}

impl Measured {
	fn of(clip: &Clip, output: &[f32]) -> Self {
		let aligned = aligned(&clip.clean, output);
		let (speech, noise) = (clip.speech(), clip.settled_noise());
		Self {
			si_sdr_before_db: si_sdr_db(&clip.clean[speech.clone()], &clip.noisy[speech.clone()]),
			si_sdr_after_db: si_sdr_db(&clip.clean[speech.clone()], &aligned[speech.clone()]),
			noise_before_db: level_db(&clip.noisy[noise.clone()]),
			noise_after_db: level_db(&aligned[noise]),
			speech_before_db: level_db(&clip.clean[speech.clone()]),
			speech_after_db: level_db(&aligned[speech]),
		}
	}

	fn noise_drop_db(&self) -> f32 {
		self.noise_before_db - self.noise_after_db
	}

	fn speech_change_db(&self) -> f32 {
		self.speech_after_db - self.speech_before_db
	}
}

impl std::fmt::Display for Measured {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			formatter,
			"SI-SDR {:.1} -> {:.1} dB, noise-only {:.1} -> {:.1} dB, speech {:.1} -> {:.1} dB",
			self.si_sdr_before_db,
			self.si_sdr_after_db,
			self.noise_before_db,
			self.noise_after_db,
			self.speech_before_db,
			self.speech_after_db
		)
	}
}

async fn joined_holding_the_key(name: &'static str, suppression: NoiseSuppression) -> Voice {
	let voice = start(name).await;
	voice
		.running
		.store()
		.update_settings(|settings| {
			settings.voice.push_to_talk = true;
			settings.voice.noise_suppression = suppression;
		})
		.expect("the settings save");
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	voice.command(Command::SetPushToTalk(true));
	voice
}

fn feed(voice: &Voice, samples: &[f32]) -> usize {
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	let frames = samples.as_chunks::<FRAME_SAMPLES>().0;
	let count = frames.len();
	for frame in frames {
		let pcm: Vec<i16> = frame
			.iter()
			.map(|sample| {
				(sample * f32::from(i16::MAX)).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
			})
			.collect();
		connection.feed_input(&pcm);
	}
	count
}

async fn suppressed_by(voice: &Voice, wanted: Option<Suppression>) {
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	let room = Clip::new(SNRS_DB[0], LEAD_IN, WARM_UP_SEED);
	feed(voice, &room.noisy[..room.lead_in]);
	voice
		.running
		.eventually("the chosen noise suppressor to run", || {
			connection.stats().suppressor == wanted
		})
		.await;
	tokio::time::sleep(QUIET_WINDOW).await;
}

async fn through_the_call(voice: &Voice, clip: &Clip) -> Vec<f32> {
	let ssrc = voice.ssrc();
	let seen = voice.running.voice().recording().packets_from(ssrc).count();
	let count = feed(voice, &clip.noisy);
	tokio::time::sleep(FRAME_TIME * count as u32 + CLIP_SETTLE).await;
	let recording = voice.running.voice().recording();
	let packets: Vec<_> = recording
		.packets_from(ssrc)
		.skip(seen)
		.filter(|packet| packet.payload != SILENCE_FRAME)
		.collect();
	let Some(first) = packets.first().map(|packet| packet.timestamp) else {
		return Vec::new();
	};
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
	output
}

async fn measured_with(voice: &Voice, suppression: NoiseSuppression, clip: &Clip) -> Measured {
	voice.change_voice_settings(|settings| settings.noise_suppression = suppression);
	tokio::time::sleep(SETTLE).await;
	Measured::of(clip, &through_the_call(voice, clip).await)
}

async fn cleans_noisy_speech(voice: &Voice, suppression: NoiseSuppression, margins_db: [f32; 3]) {
	for (snr_db, margin_db) in SNRS_DB.into_iter().zip(margins_db) {
		let clip = Clip::new(snr_db, LEAD_IN, CLIP_SEED);
		let off = measured_with(voice, NoiseSuppression::Off, &clip).await;
		let on = measured_with(voice, suppression, &clip).await;
		eprintln!("{suppression:?} at {snr_db} dB: {on}; off: {off}");
		assert!(
			on.si_sdr_after_db - off.si_sdr_after_db >= margin_db,
			"{suppression:?} at {snr_db} dB improves SI-SDR over Off by {margin_db} dB: {on}; off: {off}"
		);
		assert!(
			on.noise_drop_db() >= NOISE_DROP_DB,
			"{suppression:?} at {snr_db} dB drops noise-only stretches by {NOISE_DROP_DB} dB: {on}"
		);
		assert!(
			on.speech_change_db().abs() <= SPEECH_KEPT_DB,
			"{suppression:?} at {snr_db} dB keeps the speech level: {on}"
		);
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn high_quality_noise_suppression_is_the_default_and_cleans_noisy_speech() {
	assert_eq!(
		VoiceSettings::default().noise_suppression,
		NoiseSuppression::HighQuality,
		"high quality is the default"
	);
	let voice = joined_holding_the_key("voice-noise-high-quality", NoiseSuppression::HighQuality).await;
	suppressed_by(&voice, Some(Suppression::HighQuality)).await;
	cleans_noisy_speech(&voice, NoiseSuppression::HighQuality, HIGH_QUALITY_MARGINS_DB).await;
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn low_cpu_noise_suppression_switches_in_live_and_cleans_noisy_speech() {
	let voice = joined_holding_the_key("voice-noise-low-cpu", NoiseSuppression::HighQuality).await;
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	suppressed_by(&voice, Some(Suppression::HighQuality)).await;
	voice.change_voice_settings(|settings| settings.noise_suppression = NoiseSuppression::LowCpu);
	suppressed_by(&voice, Some(Suppression::LowCpu)).await;
	cleans_noisy_speech(&voice, NoiseSuppression::LowCpu, LOW_CPU_MARGINS_DB).await;
	let kept = voice
		.running
		.session()
		.voice_connection()
		.expect("still connected");
	assert!(Arc::ptr_eq(&kept, &connection), "switching keeps the connection");
	assert_eq!(
		identifies(&voice.running.voice().recording()).len(),
		1,
		"no second voice identify"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_noise_suppression_off_passes_the_microphone_through() {
	let voice = joined_holding_the_key("voice-noise-off", NoiseSuppression::HighQuality).await;
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	suppressed_by(&voice, Some(Suppression::HighQuality)).await;
	voice.change_voice_settings(|settings| settings.noise_suppression = NoiseSuppression::Off);
	tokio::time::sleep(SETTLE).await;
	let denoised = connection.stats().frames_denoised;
	let clip = Clip::new(SNRS_DB[1], LEAD_IN, CLIP_SEED);
	let measured = Measured::of(&clip, &through_the_call(&voice, &clip).await);
	eprintln!("off: {measured}");
	assert_eq!(
		connection.stats().frames_denoised,
		denoised,
		"no frame goes through a suppressor"
	);
	assert!(
		measured.noise_drop_db().abs() <= PASS_THROUGH_DB,
		"the noise arrives as it was: {measured}"
	);
	assert!(
		measured.speech_change_db().abs() <= PASS_THROUGH_DB,
		"the speech arrives as it was: {measured}"
	);
	voice.stop().await;
}

fn mic_level(state: &State) -> Option<f32> {
	state.mic_test().map(|test| test.level_db)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_mic_test_runs_without_joining_and_its_meter_follows_the_voice() {
	let voice = start("voice-mic-test").await;
	voice.command(Command::StartMicTest);
	voice
		.running
		.until("the mic test to start", |state| state.mic_test().is_some())
		.await;
	voice
		.running
		.eventually("the mic check to open", || {
			voice.running.session().mic_check().is_some()
		})
		.await;
	let check = voice.running.session().mic_check().expect("a running mic check");
	for index in 0..TONE_FRAMES {
		check.feed_input(&tone_frame(index));
	}
	voice
		.running
		.until("the meter to rise with the voice", |state| {
			mic_level(state).is_some_and(|level| level > MIC_TEST_SPEECH_DB)
		})
		.await;
	voice
		.running
		.eventually("the voice to be played back", || {
			check
				.take_output()
				.iter()
				.any(|frame| frame.iter().any(|sample| *sample != 0))
		})
		.await;
	voice
		.running
		.until("the meter to fall in silence", |state| {
			mic_level(state).is_some_and(|level| level < MIC_TEST_QUIET_DB)
		})
		.await;
	assert!(
		voice.running.session().state().read().voice().is_none(),
		"the mic test never joins a call"
	);
	assert!(
		voice.running.voice().recording().frames.is_empty(),
		"no voice socket was opened"
	);
	let voice_states = voice
		.running
		.mock()
		.inspect(|recording| recording.frames_with(Opcode::VoiceStateUpdate).count());
	assert_eq!(voice_states, 0, "no voice state was sent");

	voice.command(Command::StopMicTest);
	voice
		.running
		.until("the mic test to stop", |state| state.mic_test().is_none())
		.await;
	assert!(voice.running.session().mic_check().is_none());
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_mic_test_in_a_call_reads_the_call_and_joining_ends_one_outside() {
	let voice = start("voice-mic-test-in-call").await;
	voice.command(Command::StartMicTest);
	voice
		.running
		.eventually("the mic check to open", || {
			voice.running.session().mic_check().is_some()
		})
		.await;
	voice.join(voice.lounge);
	voice
		.running
		.until("joining to end the mic test", |state| state.mic_test().is_none())
		.await;
	assert!(voice.running.session().mic_check().is_none());
	voice.connected_to(voice.lounge).await;

	voice.command(Command::StartMicTest);
	voice
		.running
		.until("the mic test to start in the call", |state| {
			state.mic_test().is_some()
		})
		.await;
	voice.feed_tone();
	voice
		.running
		.until("the meter to follow the call's microphone", |state| {
			mic_level(state).is_some_and(|level| level > MIC_TEST_SPEECH_DB)
		})
		.await;
	assert!(
		voice.running.session().mic_check().is_none(),
		"no second capture opens during a call"
	);
	voice.command(Command::LeaveVoice);
	voice
		.running
		.until("leaving to end the mic test", |state| {
			state.voice().is_none() && state.mic_test().is_none()
		})
		.await;
	voice.stop().await;
}
