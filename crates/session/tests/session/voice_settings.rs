use std::sync::Arc;

use protocol::Opcode;
use session::Command;
use state::{State, VoiceStatus};
use voice::AudioDevicesInUse;

use crate::voice::{QUIET_WINDOW, SETTLE, TONE_FRAMES, identifies, manual_audio, start, tone_frame};

const QUIET_AMPLITUDE: f32 = 0.01;
const STRICT_SENSITIVITY_DB: i8 = -25;
const LOOSE_SENSITIVITY_DB: i8 = -60;
const SECOND_MIC: &str = "mock-microphone-2";
const SECOND_SPEAKERS: &str = "mock-speakers-2";
const MIC_TEST_SPEECH_DB: f32 = -30.0;
const MIC_TEST_QUIET_DB: f32 = -70.0;
const LOUDER_INPUT: u8 = 150;

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
async fn noise_suppression_toggles_the_capture_chain() {
	let voice = start("voice-noise-suppression").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("a running voice connection");
	voice.speaks(ssrc, "speech with noise suppression off").await;
	assert_eq!(connection.stats().frames_denoised, 0, "off by default");

	voice.change_voice_settings(|settings| settings.noise_suppression = true);
	tokio::time::sleep(SETTLE).await;
	voice.feed_tone();
	voice
		.running
		.eventually("frames to go through the denoiser", || {
			connection.stats().frames_denoised >= TONE_FRAMES as u64 / 2
		})
		.await;

	voice.change_voice_settings(|settings| settings.noise_suppression = false);
	tokio::time::sleep(QUIET_WINDOW).await;
	let denoised = connection.stats().frames_denoised;
	voice
		.speaks(ssrc, "speech with noise suppression off again")
		.await;
	tokio::time::sleep(QUIET_WINDOW).await;
	assert_eq!(
		connection.stats().frames_denoised,
		denoised,
		"turning it off takes the denoiser out of the chain"
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
