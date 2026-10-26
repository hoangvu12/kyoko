use std::sync::Arc;
use std::time::Duration;

use audio::Sink;
use bridge::{
	AudioFailure, AudioPhase, AudioPlayer, AudioSpeed, AudioTrack, ChannelKind, FLAT_BAR, WAVEFORM_BARS,
};
use media::{Limits, MediaService, Origin};
use mockcord::{MediaRequest, Signature, audio as fixtures};
use parking_lot::Mutex;
use protocol::fingerprint::{AUDIO_ACCEPT, AUDIO_RANGE, Fingerprint, IMAGE_REFERER};
use protocol::{Attachment, MessageFlags, Snowflake};
use tokio::sync::Notify;
use tokio::time::timeout;

use crate::harness::Running;

const WAIT: Duration = Duration::from_secs(15);
const NEAR: Duration = Duration::from_millis(60);

struct Player {
	player: AudioPlayer,
	woken: Arc<Notify>,
	refreshed: Arc<Mutex<Vec<Origin>>>,
}

impl Player {
	fn new(running: &Running) -> Self {
		let fingerprint = Fingerprint::web_client("en-US", 1);
		let fetcher = bridge::media_fetcher(&fingerprint, Some(running.server.address().port()))
			.expect("the fetcher builds");
		let service = MediaService::new(fetcher, running.scratch.join("cache"), Limits::default());
		let player = AudioPlayer::new(service, tokio::runtime::Handle::current(), Sink::Manual);
		let woken = Arc::new(Notify::new());
		let notify = woken.clone();
		player.set_waker(move || notify.notify_one());
		let refreshed = Arc::new(Mutex::new(Vec::new()));
		let seen = refreshed.clone();
		player.set_refresher(move |origin| seen.lock().push(origin));
		Self {
			player,
			woken,
			refreshed,
		}
	}

	async fn until(&self, track: &AudioTrack, phase: AudioPhase) {
		let deadline = tokio::time::Instant::now() + WAIT;
		loop {
			let woken = self.woken.notified();
			let state = self.player.state(track);
			if state.phase == phase {
				return;
			}
			if timeout(deadline - tokio::time::Instant::now(), woken)
				.await
				.is_err()
			{
				panic!("timed out waiting for {phase:?}, the track is {state:?}");
			}
		}
	}

	async fn until_duration(&self, track: &AudioTrack) -> Duration {
		let deadline = tokio::time::Instant::now() + WAIT;
		loop {
			let woken = self.woken.notified();
			if let Some(duration) = self.player.state(track).duration {
				return duration;
			}
			if timeout(deadline - tokio::time::Instant::now(), woken)
				.await
				.is_err()
			{
				panic!("timed out waiting for the duration of {}", track.filename);
			}
		}
	}

	fn position(&self, track: &AudioTrack) -> Duration {
		self.player.state(track).position
	}
}

fn near(actual: Duration, expected: Duration) -> bool {
	actual.abs_diff(expected) <= NEAR
}

fn text_channels(running: &Running) -> Vec<Snowflake> {
	running
		.view
		.sidebar
		.channels()
		.filter(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.collect()
}

async fn open(running: &mut Running, channel_id: Snowflake) {
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline
				.open()
				.is_some_and(|open| open.channel_id == channel_id)
				&& view.timeline.messages().next().is_some()
		})
		.await;
}

async fn posted(
	running: &mut Running,
	channel_id: Snowflake,
	filename: &str,
	voice: bool,
	signature: Signature,
) -> (protocol::Message, AudioTrack) {
	let message_id = running
		.server
		.post_audio(channel_id, filename, voice, signature)
		.expect("mockcord posts the audio");
	running
		.until_state("the audio message to arrive", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.by_id.contains_key(&message_id))
		})
		.await;
	let message = {
		let state = running.session().state().read();
		state.messages(channel_id).expect("the channel is cached").by_id[&message_id]
			.as_ref()
			.clone()
	};
	let track = AudioTrack::of(&message, &message.attachments[0]).expect("the attachment has a url");
	(message, track)
}

fn audio_requests(running: &Running, filename: &str) -> Vec<MediaRequest> {
	running
		.server
		.control()
		.recording()
		.media_to("/cdn/attachments/")
		.filter(|request| request.path.ends_with(filename))
		.cloned()
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_voice_message_comes_through_the_media_pipeline_and_plays_pauses_seeks_and_speeds_up() {
	let mut running = Running::start("audio-voice").await;
	let channel_id = text_channels(&running)[0];
	open(&mut running, channel_id).await;
	let player = Player::new(&running);
	let (message, track) = posted(
		&mut running,
		channel_id,
		fixtures::VOICE_MESSAGE_FILENAME,
		true,
		Signature::Fresh,
	)
	.await;
	let attachment = &message.attachments[0];
	assert!(message.flags.contains(MessageFlags::IS_VOICE_MESSAGE));
	assert!(bridge::is_voice_message(&message, attachment));
	let bars = bridge::waveform(attachment, WAVEFORM_BARS);
	assert_eq!(bars.len(), WAVEFORM_BARS);
	assert!(
		(bars[0] - 0.3).abs() < 0.01 && (bars[5] - 1.).abs() < 0.01,
		"{bars:?}"
	);

	let idle = player.player.state(&track);
	assert_eq!(idle.phase, AudioPhase::Idle);
	assert_eq!(
		idle.duration,
		Some(Duration::from_secs(8)),
		"the payload's duration shows before loading"
	);
	assert!(audio_requests(&running, fixtures::VOICE_MESSAGE_FILENAME).is_empty());

	player.player.toggle(&track);
	player.until(&track, AudioPhase::Playing).await;
	let fetched = audio_requests(&running, fixtures::VOICE_MESSAGE_FILENAME);
	assert_eq!(fetched.len(), 1, "one fetch through the media pipeline");
	let request = &fetched[0];
	assert_eq!(request.status, 200);
	assert_eq!(request.header("accept"), Some(AUDIO_ACCEPT));
	assert_eq!(request.header("range"), Some(AUDIO_RANGE));
	assert_eq!(request.header("referer"), Some(IMAGE_REFERER));
	assert_eq!(request.header("sec-fetch-dest"), Some("audio"));
	assert_eq!(request.header("sec-fetch-mode"), Some("no-cors"));
	assert!(
		request
			.header("user-agent")
			.is_some_and(|agent| agent.contains("Chrome/"))
	);
	assert!(request.header("authorization").is_none());
	assert!(request.header("cookie").is_none());

	player.player.pump(Duration::from_secs(2));
	assert!(
		near(player.position(&track), Duration::from_secs(2)),
		"{:?}",
		player.position(&track)
	);

	assert_eq!(player.player.cycle_speed(&track), AudioSpeed::Faster);
	assert_eq!(AudioSpeed::Faster.label(), "1.5x");
	player.player.pump(Duration::from_secs(2));
	assert!(
		near(player.position(&track), Duration::from_secs(5)),
		"{:?}",
		player.position(&track)
	);

	player.player.toggle(&track);
	assert_eq!(player.player.state(&track).phase, AudioPhase::Paused);
	player.player.pump(Duration::from_secs(2));
	assert!(
		near(player.position(&track), Duration::from_secs(5)),
		"a paused track stays put"
	);

	player.player.seek(&track, 0.25);
	assert!(near(player.position(&track), Duration::from_secs(2)));
	assert_eq!(player.player.cycle_speed(&track), AudioSpeed::Fastest);
	player.player.toggle(&track);
	assert_eq!(player.player.state(&track).phase, AudioPhase::Playing);
	player.player.pump(Duration::from_secs(1));
	assert!(
		near(player.position(&track), Duration::from_secs(4)),
		"{:?}",
		player.position(&track)
	);
	assert!((player.player.state(&track).progress() - 0.5).abs() < 0.02);

	assert!(player.player.toggle_mute(&track));
	assert!(player.player.state(&track).muted);
	player.player.pump(Duration::from_secs(3));
	let ended = player.player.state(&track);
	assert_eq!(ended.phase, AudioPhase::Idle, "the end rewinds");
	assert_eq!(ended.position, Duration::ZERO);
	assert_eq!(ended.speed, AudioSpeed::Fastest, "the speed stays chosen");

	assert_eq!(player.player.cycle_speed(&track), AudioSpeed::Normal);
	player.player.toggle(&track);
	assert_eq!(player.player.state(&track).phase, AudioPhase::Playing);
	assert_eq!(
		audio_requests(&running, fixtures::VOICE_MESSAGE_FILENAME).len(),
		1,
		"a replay reads the cached bytes"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_one_track_plays_at_a_time_and_each_keeps_its_place() {
	let mut running = Running::start("audio-one-at-a-time").await;
	let channel_id = text_channels(&running)[0];
	open(&mut running, channel_id).await;
	let player = Player::new(&running);
	let (_, voice) = posted(
		&mut running,
		channel_id,
		fixtures::VOICE_MESSAGE_FILENAME,
		true,
		Signature::Fresh,
	)
	.await;
	let (file_message, file) = posted(
		&mut running,
		channel_id,
		fixtures::AUDIO_FILENAME,
		false,
		Signature::Fresh,
	)
	.await;
	assert!(bridge::is_audio(&file_message.attachments[0]));
	assert!(!bridge::is_voice_message(
		&file_message,
		&file_message.attachments[0]
	));
	assert_eq!(file.duration, None, "an audio file carries no duration");
	player.player.probe(&file);
	assert_eq!(
		player.until_duration(&file).await,
		Duration::from_secs(fixtures::AUDIO_FILE_SECS as u64),
		"the file's length is read through the pipeline"
	);

	player.player.toggle(&voice);
	player.until(&voice, AudioPhase::Playing).await;
	player.player.pump(Duration::from_secs(1));

	player.player.toggle(&file);
	assert_eq!(player.player.state(&voice).phase, AudioPhase::Paused);
	assert!(near(player.position(&voice), Duration::from_secs(1)));
	player.until(&file, AudioPhase::Playing).await;
	assert_eq!(player.player.playing(), Some(file.id));
	player.player.pump(Duration::from_secs(3));
	assert!(
		near(player.position(&voice), Duration::from_secs(1)),
		"only the active track moves"
	);
	assert!(
		near(player.position(&file), Duration::from_secs(3)),
		"{:?}",
		player.position(&file)
	);

	player.player.toggle(&voice);
	assert_eq!(player.player.state(&file).phase, AudioPhase::Paused);
	player.until(&voice, AudioPhase::Playing).await;
	assert_eq!(player.player.playing(), Some(voice.id));
	player.player.pump(Duration::from_millis(500));
	assert!(
		near(player.position(&voice), Duration::from_millis(1500)),
		"the voice message resumes where it paused: {:?}",
		player.position(&voice)
	);
	assert!(near(player.position(&file), Duration::from_secs(3)));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaving_the_channel_stops_playback() {
	let mut running = Running::start("audio-leave").await;
	let channels = text_channels(&running);
	let (first, second) = (channels[0], channels[1]);
	open(&mut running, first).await;
	let player = Player::new(&running);
	let (_, track) = posted(
		&mut running,
		first,
		fixtures::VOICE_MESSAGE_FILENAME,
		true,
		Signature::Fresh,
	)
	.await;
	player.player.toggle(&track);
	player.until(&track, AudioPhase::Playing).await;
	player.player.pump(Duration::from_secs(1));

	player.player.left(second);
	assert_eq!(
		player.player.playing(),
		Some(track.id),
		"another channel closing changes nothing"
	);

	let left = running.view.timeline.open().map(|open| open.channel_id);
	open(&mut running, second).await;
	let now = running.view.timeline.open().map(|open| open.channel_id);
	assert_ne!(left, now);
	if let Some(left) = left {
		player.player.left(left);
	}
	assert_eq!(player.player.playing(), None);
	let stopped = player.player.state(&track);
	assert_eq!(stopped.phase, AudioPhase::Idle);
	assert_eq!(stopped.position, Duration::ZERO, "coming back starts over");
	player.player.pump(Duration::from_secs(1));
	assert_eq!(player.player.state(&track).position, Duration::ZERO);

	open(&mut running, first).await;
	player.player.toggle(&track);
	player.until(&track, AudioPhase::Playing).await;
	player.player.stop();
	assert_eq!(player.player.playing(), None, "signing out stops it too");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_broken_file_shows_an_error_and_a_bad_waveform_draws_flat() {
	let mut running = Running::start("audio-broken").await;
	let channel_id = text_channels(&running)[0];
	open(&mut running, channel_id).await;
	let player = Player::new(&running);

	let flat = vec![FLAT_BAR; WAVEFORM_BARS];
	let mut attachment = Attachment {
		waveform: Some("%%% not base64 %%%".to_owned()),
		..Attachment::default()
	};
	assert_eq!(bridge::waveform(&attachment, WAVEFORM_BARS), flat);
	attachment.waveform = None;
	assert_eq!(bridge::waveform(&attachment, WAVEFORM_BARS), flat);
	attachment.waveform = Some(String::new());
	assert_eq!(bridge::waveform(&attachment, WAVEFORM_BARS), flat);
	attachment.waveform = Some("AP8".to_owned());
	let short = bridge::waveform(&attachment, WAVEFORM_BARS);
	assert_eq!(short.len(), WAVEFORM_BARS);
	assert!(short.iter().all(|bar| (FLAT_BAR..=1.).contains(bar)));

	let (_, broken) = posted(
		&mut running,
		channel_id,
		fixtures::BROKEN_AUDIO_FILENAME,
		true,
		Signature::Fresh,
	)
	.await;
	player.player.toggle(&broken);
	player.until(&broken, AudioPhase::Failed).await;
	assert_eq!(
		player.player.state(&broken).failure,
		Some(AudioFailure::Unplayable)
	);
	assert_eq!(player.player.playing(), None);

	let (expired_message, expired) = posted(
		&mut running,
		channel_id,
		fixtures::VOICE_REPLY_FILENAME,
		true,
		Signature::Revoked,
	)
	.await;
	player.player.toggle(&expired);
	player.until(&expired, AudioPhase::Failed).await;
	assert_eq!(
		player.player.state(&expired).failure,
		Some(AudioFailure::Unavailable)
	);
	assert_eq!(
		*player.refreshed.lock(),
		vec![Origin {
			channel_id: channel_id.0,
			message_id: expired_message.id.0,
		}],
		"a gone link asks for a fresh one"
	);

	player.player.toggle(&broken);
	assert_eq!(
		player.player.state(&broken).phase,
		AudioPhase::Loading,
		"a failed track can be retried"
	);
	player.until(&broken, AudioPhase::Failed).await;
	running.stop().await;
}
