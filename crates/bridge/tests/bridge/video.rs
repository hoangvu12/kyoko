use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use audio::Sink;
use bridge::{ChannelKind, VideoClip, VideoFailure, VideoPhase, VideoPicture, VideoPlayer, VideoState};
use media::{Limits, MediaService, Origin, VideoLimits};
use mockcord::{MediaRequest, Signature, clip};
use parking_lot::Mutex;
use protocol::fingerprint::{Fingerprint, IMAGE_REFERER, VIDEO_ACCEPT, VIDEO_ACCEPT_ENCODING};
use protocol::{Embed, EmbedKind, EmbedMedia, LenientVec, Message, Snowflake};
use tokio::sync::Notify;
use tokio::time::{Instant, sleep, timeout};

use crate::harness::Running;

const WAIT: Duration = Duration::from_secs(20);
const SMALL_READ_AHEAD: u64 = 8 * 1024;
const VIEW: (u32, u32) = (160, 90);

struct Player {
	player: VideoPlayer,
	woken: Arc<Notify>,
	pictures: Arc<Mutex<Vec<VideoPicture>>>,
	refreshed: Arc<Mutex<Vec<Origin>>>,
}

impl Player {
	fn new(running: &Running, limits: VideoLimits) -> Self {
		let fingerprint = Fingerprint::web_client("en-US", 1);
		let fetcher = bridge::media_fetcher(&fingerprint, Some(running.server.address().port()))
			.expect("the fetcher builds");
		let service = MediaService::new(fetcher, running.scratch.join("cache"), Limits::default());
		let player = VideoPlayer::new(service, tokio::runtime::Handle::current(), Sink::Silent);
		player.set_limits(limits);
		let woken = Arc::new(Notify::new());
		let notify = woken.clone();
		player.set_waker(move || notify.notify_one());
		let refreshed = Arc::new(Mutex::new(Vec::new()));
		let seen = refreshed.clone();
		player.set_refresher(move |origin| seen.lock().push(origin));
		Self {
			player,
			woken,
			pictures: Arc::default(),
			refreshed,
		}
	}

	async fn until(&self, what: &str, clip: &VideoClip, done: impl Fn(&VideoState) -> bool) -> VideoState {
		let deadline = Instant::now() + WAIT;
		loop {
			let woken = self.woken.notified();
			self.pictures.lock().extend(self.player.drain());
			let state = self.player.state(clip);
			if done(&state) {
				return state;
			}
			if timeout(deadline - Instant::now(), woken).await.is_err() {
				panic!("timed out waiting for {what}; the clip is {state:?}");
			}
		}
	}

	fn pictures_of(&self, clip: &VideoClip) -> Vec<VideoPicture> {
		self.pictures.lock().extend(self.player.drain());
		self.pictures
			.lock()
			.iter()
			.filter(|picture| picture.key == clip.key)
			.cloned()
			.collect()
	}
}

fn text_channel(running: &Running) -> Snowflake {
	running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel")
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

async fn posted(running: &mut Running, signature: Signature) -> (Message, VideoClip) {
	let channel_id = text_channel(running);
	let open_already = running
		.view
		.timeline
		.open()
		.is_some_and(|open| open.channel_id == channel_id);
	if !open_already {
		open(running, channel_id).await;
	}
	let message_id = running
		.server
		.post_video(channel_id, signature)
		.expect("mockcord posts the video");
	running
		.until_state("the video message to arrive", |state| {
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
	let clip = VideoClip::attachment(&message, &message.attachments[0]).expect("a playable attachment");
	(message, clip)
}

fn video_requests(running: &Running) -> Vec<MediaRequest> {
	running
		.server
		.control()
		.recording()
		.media_to("/cdn/attachments/")
		.filter(|request| request.path.ends_with(clip::CLIP_FILENAME) && request.param("format").is_none())
		.cloned()
		.collect()
}

fn range_start(request: &MediaRequest) -> u64 {
	request
		.header("range")
		.and_then(|range| range.strip_prefix("bytes="))
		.and_then(|range| range.strip_suffix('-'))
		.and_then(|start| start.parse().ok())
		.unwrap_or_else(|| panic!("a bytes=N- range, got {:?}", request.header("range")))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_video_attachment_streams_in_ranges_and_plays_pauses_seeks_and_ends() {
	let mut running = Running::start("video-attachment").await;
	let player = Player::new(
		&running,
		VideoLimits {
			read_ahead: SMALL_READ_AHEAD,
			..VideoLimits::default()
		},
	);
	let (_, clip) = posted(&mut running, Signature::Fresh).await;
	let idle = player.player.state(&clip);
	assert_eq!(idle.phase, VideoPhase::Idle);
	assert_eq!(
		idle.duration,
		Some(Duration::from_secs(3)),
		"the payload's duration shows first"
	);
	assert!(
		video_requests(&running).is_empty(),
		"nothing is fetched before play"
	);

	player.player.set_view(&clip, VIEW.0, VIEW.1);
	player.player.play(&clip);
	player
		.until("the first frame", &clip, |state| {
			state.phase == VideoPhase::Playing && state.shown
		})
		.await;
	let shown = player.pictures_of(&clip);
	let first = shown.first().expect("a picture for the clip");
	assert_eq!(
		(first.picture.width, first.picture.height),
		VIEW,
		"frames are converted at the view's size"
	);
	player
		.until("playback to move", &clip, |state| {
			state.position >= Duration::from_millis(800)
		})
		.await;

	let requests = video_requests(&running);
	assert!(
		requests.len() > 1,
		"the clip is fetched in several ranges, not whole: {requests:?}"
	);
	let first = &requests[0];
	assert_eq!(
		first.header("range"),
		Some("bytes=0-"),
		"Chrome starts a <video> at bytes=0-"
	);
	assert_eq!(first.status, 206);
	assert_eq!(first.header("accept"), Some(VIDEO_ACCEPT));
	assert_eq!(first.header("accept-encoding"), Some(VIDEO_ACCEPT_ENCODING));
	assert_eq!(first.header("sec-fetch-dest"), Some("video"));
	assert_eq!(first.header("sec-fetch-mode"), Some("no-cors"));
	assert_eq!(first.header("sec-fetch-site"), Some("cross-site"));
	assert_eq!(first.header("referer"), Some(IMAGE_REFERER));
	assert!(first.header("authorization").is_none());
	assert!(first.header("cookie").is_none());
	let starts: Vec<u64> = requests.iter().map(range_start).collect();
	assert!(
		starts.windows(2).all(|pair| pair[0] < pair[1]),
		"each range continues where the last stopped: {starts:?}"
	);

	player.player.pause(&clip);
	let paused = player
		.until("the pause", &clip, |state| state.phase == VideoPhase::Paused)
		.await;
	sleep(Duration::from_millis(400)).await;
	assert!(
		player.player.state(&clip).position.abs_diff(paused.position) <= Duration::from_millis(100),
		"a paused clip stays still"
	);

	player.player.seek(&clip, 0.5);
	player
		.until("the seek", &clip, |state| {
			state.position.abs_diff(Duration::from_millis(1500)) <= Duration::from_millis(150)
		})
		.await;
	assert_eq!(
		player.player.state(&clip).phase,
		VideoPhase::Paused,
		"seeking keeps it paused"
	);

	player.player.play(&clip);
	let ended = player
		.until("the end", &clip, |state| state.phase == VideoPhase::Ended)
		.await;
	assert!(ended.position >= Duration::from_millis(2800), "{ended:?}");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_video_over_the_size_cap_and_an_expired_one_fail_without_playing() {
	let mut running = Running::start("video-limits").await;
	let capped = Player::new(
		&running,
		VideoLimits {
			max_bytes: 4 * 1024,
			..VideoLimits::default()
		},
	);
	let (_, clip) = posted(&mut running, Signature::Fresh).await;
	capped.player.play(&clip);
	let failed = capped
		.until("the cap", &clip, |state| state.phase == VideoPhase::Failed)
		.await;
	assert_eq!(failed.failure, Some(VideoFailure::Unavailable));
	assert!(!failed.shown);

	let player = Player::new(&running, VideoLimits::default());
	let (message, revoked) = posted(&mut running, Signature::Revoked).await;
	player.player.play(&revoked);
	let failed = player
		.until("the refusal", &revoked, |state| state.phase == VideoPhase::Failed)
		.await;
	assert_eq!(failed.failure, Some(VideoFailure::Unavailable));
	assert_eq!(
		player.refreshed.lock().clone(),
		vec![Origin {
			channel_id: message.channel_id.0,
			message_id: message.id.0,
		}],
		"a gone attachment asks for a fresh link"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gif_embed_plays_muted_loops_and_pauses_when_it_leaves_the_screen() {
	let running = Running::start("video-gifv").await;
	let player = Player::new(&running, VideoLimits::default());
	let origin = format!("http://127.0.0.1:{}", running.server.address().port());
	let video = "https://media.tenor.com/kYoKoGiFvAAAPo/kyoko.mp4";
	let message = Message {
		id: Snowflake(91),
		channel_id: Snowflake(9),
		embeds: LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Gifv),
			video: Some(EmbedMedia {
				url: Some(video.to_owned()),
				proxy_url: None,
				width: Some(clip::CLIP_WIDTH),
				height: Some(clip::CLIP_HEIGHT),
				content_type: None,
				placeholder: None,
				flags: None,
			}),
			..Embed::default()
		}]),
		..Message::default()
	};
	let mut gif = VideoClip::embed(&message, 0, &message.embeds[0]).expect("a playable gif embed");
	assert!(gif.gifv);
	gif.url = bridge::local_video(&gif.url, Some(&origin));
	let youtube = Embed {
		video: Some(EmbedMedia {
			url: Some("https://www.youtube.com/embed/kyoko".to_owned()),
			proxy_url: Some(
				"https://images-ext-1.discordapp.net/external/x/https/www.youtube.com/embed/kyoko".to_owned(),
			),
			..message.embeds[0].video.clone().expect("a video")
		}),
		..Embed::default()
	};
	assert!(
		!bridge::is_playable_embed(&youtube),
		"iframe players keep opening their link"
	);

	player.player.play(&gif);
	let playing = player.until("the gif to play", &gif, |state| state.shown).await;
	assert!(playing.muted, "GIF embeds play muted");
	let highest = Cell::new(Duration::ZERO);
	player
		.until("the gif to loop", &gif, |state| {
			let looped =
				highest.get() >= Duration::from_millis(2500) && state.position < Duration::from_secs(1);
			highest.set(highest.get().max(state.position));
			looped
		})
		.await;
	assert!(
		player.player.state(&gif).is_playing(),
		"a gif keeps playing after its end"
	);

	player.player.set_visible(std::iter::empty());
	player
		.until("the pause off screen", &gif, |state| {
			state.phase == VideoPhase::Paused
		})
		.await;
	running.stop().await;
}
