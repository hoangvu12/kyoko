use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::ChannelKind;
use media::cdn::Quality;
use media::{
	Gallery, ImageName, ImageRequest, Limits, MediaService, Playback, Shape, SizeHint, Status, Update,
	Variant,
};
use mockcord::{ANIMATION_FRAMES, FRAME_DELAY_MS, MediaRequest, Signature};
use protocol::fingerprint::{Fingerprint, IMAGE_ACCEPT, IMAGE_REFERER, sec_ch_ua};
use protocol::{Attachment, Snowflake};
use tokio::sync::Notify;
use tokio::time::timeout;

use crate::harness::Running;

const WAIT: Duration = Duration::from_secs(15);
const FRAME: Duration = Duration::from_millis(FRAME_DELAY_MS as u64);

struct Images {
	gallery: Gallery,
	woken: Arc<Notify>,
}

impl Images {
	fn new(running: &Running, budget: usize) -> Self {
		let fingerprint = Fingerprint::web_client("en-US", 1);
		let fetcher = bridge::media_fetcher(&fingerprint, Some(running.server.address().port()))
			.expect("the fetcher builds");
		let loader = MediaService::new(fetcher, running.scratch.join("cache"), Limits::default());
		let gallery = Gallery::with_budget(loader, tokio::runtime::Handle::current(), budget);
		let woken = Arc::new(Notify::new());
		let notify = woken.clone();
		gallery.set_waker(move || notify.notify_one());
		Self { gallery, woken }
	}

	async fn settled(&self, name: &ImageName) -> Status {
		let deadline = tokio::time::Instant::now() + WAIT;
		loop {
			let woken = self.woken.notified();
			if let Some(status @ (Status::Ready { .. } | Status::Failed | Status::Refreshing)) =
				self.gallery.status(name)
			{
				return status;
			}
			if timeout(deadline - tokio::time::Instant::now(), woken)
				.await
				.is_err()
			{
				panic!("timed out waiting for {name}");
			}
		}
	}

	async fn ready(&self, request: ImageRequest) -> ImageName {
		let name = self.gallery.request(request);
		let status = self.settled(&name).await;
		assert!(matches!(status, Status::Ready { .. }), "{name} ended {status:?}");
		name
	}
}

fn shown<'a>(updates: &'a [Update], name: &ImageName) -> Option<&'a media::Pixels> {
	updates.iter().find_map(|update| match update {
		Update::Show { name: shown, pixels } if shown == name => Some(pixels),
		_ => None,
	})
}

fn cleared(updates: &[Update], name: &ImageName) -> bool {
	updates
		.iter()
		.any(|update| matches!(update, Update::Clear { name: gone } if gone == name))
}

fn square(url: String, side: u32) -> ImageRequest {
	ImageRequest::new(url, Variant::new(SizeHint::square(side), Shape::Square))
}

fn media_requests(running: &Running, prefix: &str) -> Vec<MediaRequest> {
	running
		.server
		.control()
		.recording()
		.media_to(prefix)
		.cloned()
		.collect()
}

fn first_text_channel(running: &Running) -> Snowflake {
	running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("the first guild has a text channel")
}

async fn open_first_channel(running: &mut Running) -> Snowflake {
	let channel_id = first_text_channel(running);
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	channel_id
}

async fn posted_attachment(
	running: &mut Running,
	channel_id: Snowflake,
	filename: &str,
	signature: Signature,
) -> (Snowflake, Attachment) {
	let message_id = running
		.server
		.post_image(channel_id, filename, signature)
		.expect("mockcord posts the image");
	running
		.until_state("the image message to arrive", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.by_id.contains_key(&message_id))
		})
		.await;
	(message_id, attachment_of(running, channel_id, message_id))
}

fn attachment_of(running: &Running, channel_id: Snowflake, message_id: Snowflake) -> Attachment {
	let state = running.session().state().read();
	state.messages(channel_id).expect("the channel is cached").by_id[&message_id].attachments[0].clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_format_decodes_into_host_images() {
	let running = Running::start("media-formats").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let cdn = running.server.cdn_url();
	let cases = [
		(format!("{cdn}/avatars/1/plain.png?size=64"), 1),
		(format!("{cdn}/avatars/1/plain.jpg?size=64"), 1),
		(format!("{cdn}/avatars/1/plain.webp?size=64"), 1),
		(format!("{cdn}/avatars/1/a_moving.gif?size=64"), ANIMATION_FRAMES),
		(
			format!("{cdn}/avatars/1/a_moving.webp?size=64&animated=true"),
			ANIMATION_FRAMES,
		),
	];
	let mut names = Vec::new();
	for (url, frames) in cases {
		let name = images.ready(square(url.clone(), 64)).await;
		assert_eq!(
			images.gallery.status(&name),
			Some(Status::Ready { frames }),
			"{url} decodes with {frames} frames"
		);
		names.push(name);
	}
	let updates = images.gallery.drain(Instant::now());
	for name in &names {
		let pixels = shown(&updates, name).expect("each image reaches the host");
		assert_eq!((pixels.width, pixels.height), (64, 64));
		assert_eq!(pixels.rgba.len(), 64 * 64 * 4);
	}
	assert!(
		images.gallery.drain(Instant::now()).is_empty(),
		"a shown frame is not sent twice"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn avatars_are_sized_in_device_pixels_rounded_and_fetched_like_chrome() {
	let running = Running::start("media-avatar").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let user = running
		.session()
		.state()
		.read()
		.current_user()
		.cloned()
		.expect("READY names the signed-in user");
	let request = bridge::avatar_request(&running.server.cdn_url(), &user, 32., 1.5, false)
		.expect("the mock user has an avatar");
	assert!(request.url.ends_with(".webp?size=64"), "{}", request.url);
	let name = images.ready(request).await;
	let updates = images.gallery.drain(Instant::now());
	let pixels = shown(&updates, &name).expect("the avatar reaches the host");
	assert_eq!((pixels.width, pixels.height), (64, 64));
	let alpha = |x: usize, y: usize| pixels.rgba[(y * 64 + x) * 4 + 3];
	assert_eq!(alpha(0, 0), 0, "the corners are cut away");
	assert_eq!(alpha(63, 63), 0);
	assert_eq!(alpha(32, 32), u8::MAX, "the middle stays opaque");

	let fetched = media_requests(&running, "/cdn/avatars/");
	assert_eq!(fetched.len(), 1);
	let request = &fetched[0];
	assert_eq!(request.param("size"), Some("64"));
	assert_eq!(request.header("accept"), Some(IMAGE_ACCEPT));
	assert_eq!(request.header("referer"), Some(IMAGE_REFERER));
	assert_eq!(request.header("sec-fetch-dest"), Some("image"));
	assert_eq!(request.header("sec-fetch-mode"), Some("no-cors"));
	assert_eq!(request.header("sec-fetch-site"), Some("cross-site"));
	assert_eq!(request.header("sec-ch-ua"), Some(sec_ch_ua().as_str()));
	assert_eq!(request.header("sec-ch-ua-mobile"), Some("?0"));
	assert!(
		request
			.header("user-agent")
			.is_some_and(|agent| agent.contains("Chrome/"))
	);
	assert!(request.header("authorization").is_none());
	assert!(request.header("cookie").is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_same_image_in_flight_is_fetched_once() {
	let running = Running::start("media-coalesce").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let url = format!("{}/avatars/7/shared.webp?size=128", running.server.cdn_url());
	let first = images.gallery.request(square(url.clone(), 128));
	let again = images.gallery.request(square(url.clone(), 128));
	assert_eq!(first, again, "one image, one host name");
	assert!(matches!(images.settled(&first).await, Status::Ready { .. }));

	let other = format!("{}/avatars/7/other.webp?size=128", running.server.cdn_url());
	let loader = images.gallery.loader().clone();
	let square = Variant::new(SizeHint::square(128), Shape::Square);
	let round = Variant::new(SizeHint::square(128), Shape::Circle);
	let (left, right) = tokio::join!(
		loader.load_variant(&other, square, media::Source::Anywhere),
		loader.load_variant(&other, round, media::Source::Anywhere),
	);
	assert!(left.expect("the square loads").is_some());
	assert!(right.expect("the circle loads").is_some());
	assert_eq!(media_requests(&running, "/cdn/avatars/7/shared.webp").len(), 1);
	assert_eq!(media_requests(&running, "/cdn/avatars/7/other.webp").len(), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_small_budget_evicts_the_least_recently_shown_image() {
	let running = Running::start("media-budget").await;
	let one = 64 * 64 * 4;
	let images = Images::new(&running, one * 3 / 2);
	let cdn = running.server.cdn_url();
	let first = images
		.ready(square(format!("{cdn}/avatars/1/first.png?size=64"), 64))
		.await;
	assert!(shown(&images.gallery.drain(Instant::now()), &first).is_some());

	let second = images
		.ready(square(format!("{cdn}/avatars/1/second.png?size=64"), 64))
		.await;
	let updates = images.gallery.drain(Instant::now());
	assert!(cleared(&updates, &first), "the older image leaves the host");
	assert!(shown(&updates, &second).is_some());
	assert_eq!(images.gallery.status(&first), None);
	let stats = images.gallery.stats();
	assert!(stats.bytes <= stats.budget);
	assert_eq!(stats.entries, 1);

	images.gallery.set_budget(one * 5 / 2);
	let third = images
		.ready(square(format!("{cdn}/avatars/1/third.png?size=64"), 64))
		.await;
	images.gallery.set_visible([second.as_str()]);
	assert!(shown(&images.gallery.drain(Instant::now()), &third).is_some());
	let fourth = images
		.ready(square(format!("{cdn}/avatars/1/fourth.png?size=64"), 64))
		.await;
	let updates = images.gallery.drain(Instant::now());
	assert!(
		cleared(&updates, &third),
		"the image shown least recently goes, not the visible one"
	);
	assert!(!cleared(&updates, &second));
	assert!(shown(&updates, &fourth).is_some());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attachments_ask_the_media_proxy_for_device_pixels() {
	let mut running = Running::start("media-proxy").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let channel_id = open_first_channel(&mut running).await;
	let (message_id, attachment) =
		posted_attachment(&mut running, channel_id, "photo.png", Signature::Fresh).await;
	let request = bridge::attachment_request(
		&attachment,
		channel_id,
		message_id,
		(400., 300.),
		1.25,
		Quality::Fitted,
	)
	.expect("a png is an image");
	images.ready(request).await;
	let fetched = media_requests(&running, "/cdn/attachments/");
	assert_eq!(fetched.len(), 1);
	let request = &fetched[0];
	assert_eq!(request.param("format"), Some("webp"));
	assert_eq!(request.param("quality"), Some("lossless"));
	assert_eq!(request.param("width"), Some("500"));
	assert_eq!(request.param("height"), Some("375"));
	assert!(request.param("ex").is_some() && request.param("hm").is_some());

	let original = bridge::attachment_request(
		&attachment,
		channel_id,
		message_id,
		(400., 300.),
		1.25,
		Quality::Original,
	)
	.expect("a png is an image");
	assert_eq!(Some(&original.url), attachment.url.as_ref());
	images.ready(original).await;
	let fetched = media_requests(&running, "/cdn/attachments/");
	assert_eq!(fetched.len(), 2);
	assert_eq!(fetched[1].param("width"), None);
	assert_eq!(fetched[1].param("format"), None);
	running.stop().await;
}

async fn refreshed_through_the_message(name: &str, signature: Signature, stale_fetches: usize) {
	let mut running = Running::start(name).await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let commander = running.session().commander();
	images
		.gallery
		.set_refresher(move |origin| commander.send(bridge::refresh_command(origin)));
	let channel_id = open_first_channel(&mut running).await;
	let (message_id, stale) = posted_attachment(&mut running, channel_id, "old.png", signature).await;
	let stale_url = stale.proxy_url.clone().expect("a proxy url");
	let request = |attachment: &Attachment| {
		bridge::attachment_request(
			attachment,
			channel_id,
			message_id,
			(320., 240.),
			1.,
			Quality::Fitted,
		)
		.expect("a png is an image")
	};
	let first = images.gallery.request(request(&stale));
	assert_eq!(images.settled(&first).await, Status::Refreshing);
	running
		.until_state("the message to come back with a fresh url", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&message_id))
				.and_then(|message| message.attachments.first())
				.is_some_and(|attachment| attachment.proxy_url.as_deref() != Some(stale_url.as_str()))
		})
		.await;
	let fresh = attachment_of(&running, channel_id, message_id);
	let again = images.gallery.request(request(&fresh));
	assert_eq!(first, again, "the refreshed image keeps its host name");
	assert!(matches!(images.settled(&again).await, Status::Ready { .. }));
	assert!(shown(&images.gallery.drain(Instant::now()), &again).is_some());

	let recording = running.server.control().recording();
	let refetches: Vec<_> = recording
		.requests_to("GET", "/channels/*/messages")
		.filter(|request| {
			request
				.query
				.as_deref()
				.is_some_and(|query| query.contains(&format!("around={message_id}")))
		})
		.collect();
	assert_eq!(refetches.len(), 1, "the message is fetched again once");
	let stale_hm = stale_url
		.split("hm=")
		.nth(1)
		.map(|rest| rest.split('&').next().unwrap_or(rest));
	let fetched: Vec<_> = recording.media_to("/cdn/attachments/").collect();
	let stale_tries = fetched
		.iter()
		.filter(|request| request.param("hm") == stale_hm)
		.count();
	assert_eq!(stale_tries, stale_fetches, "the stale url is never retried");
	assert_eq!(fetched.len(), stale_fetches + 1);
	assert_eq!(fetched.last().map(|request| request.status), Some(200));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_attachment_url_is_refreshed_by_refetching_its_message() {
	refreshed_through_the_message("media-expired", Signature::Expired, 0).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_signed_url_is_refreshed_once_by_refetching_its_message() {
	refreshed_through_the_message("media-revoked", Signature::Revoked, 1).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn animations_play_only_while_visible_and_allowed() {
	let running = Running::start("media-animation").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let url = format!("{}/avatars/3/a_dance.gif?size=32", running.server.cdn_url());
	let name = images.ready(square(url, 32)).await;
	let start = Instant::now();
	let frame_at = |updates: &[Update]| shown(updates, &name).map(|pixels| pixels.rgba.clone());
	let first = frame_at(&images.gallery.drain(start)).expect("the first frame shows at once");

	assert!(
		images.gallery.drain(start + FRAME * 3).is_empty(),
		"an animation nobody sees stays put"
	);
	assert_eq!(images.gallery.next_due(), None);

	images.gallery.set_visible([name.as_str()]);
	let base = start + FRAME * 4;
	assert!(images.gallery.drain(base).is_empty());
	assert_eq!(images.gallery.next_due(), Some(base + FRAME));
	let second = frame_at(&images.gallery.drain(base + FRAME)).expect("the next frame follows its delay");
	assert_ne!(second, first);
	let third = frame_at(&images.gallery.drain(base + FRAME * 2)).expect("and the one after");
	assert_ne!(third, second);

	images.gallery.set_visible(Vec::<&str>::new());
	assert!(
		images.gallery.drain(base + FRAME * 10).is_empty(),
		"scrolled away, it pauses on its frame"
	);
	assert_eq!(images.gallery.next_due(), None);

	images.gallery.set_visible([name.as_str()]);
	images.gallery.set_reduced_motion(true);
	let reset = images.gallery.drain(base + FRAME * 11);
	assert_eq!(
		frame_at(&reset),
		Some(first.clone()),
		"reduced motion shows the first frame"
	);
	assert!(images.gallery.drain(base + FRAME * 20).is_empty());
	images.gallery.set_reduced_motion(false);

	images.gallery.set_playback(Playback::Never);
	assert!(images.gallery.drain(base + FRAME * 30).is_empty());
	assert!(images.gallery.drain(base + FRAME * 40).is_empty());

	images.gallery.set_playback(Playback::OnHover);
	assert!(images.gallery.drain(base + FRAME * 50).is_empty());
	images.gallery.set_hovered(Some(name.as_str()));
	let hover = base + FRAME * 60;
	assert!(images.gallery.drain(hover).is_empty());
	assert!(
		frame_at(&images.gallery.drain(hover + FRAME)).is_some(),
		"hovering plays it"
	);
	images.gallery.set_hovered(None);
	assert_eq!(
		frame_at(&images.gallery.drain(hover + FRAME * 2)),
		Some(first),
		"leaving it returns to the first frame"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_animation_and_reduced_motion_settings_hold_animations_still() {
	let running = Running::start("media-setting-animation").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let url = format!("{}/avatars/3/a_still.gif?size=32", running.server.cdn_url());
	let name = images.ready(square(url, 32)).await;
	let start = Instant::now();
	let frame_at = |updates: &[Update]| shown(updates, &name).map(|pixels| pixels.rgba.clone());
	let first = frame_at(&images.gallery.drain(start)).expect("the first frame shows at once");
	images.gallery.set_visible([name.as_str()]);
	let mut settings = store::Settings::default();
	images.gallery.set_playback(bridge::media_playback(&settings));
	images.gallery.drain(start + FRAME);
	assert!(
		frame_at(&images.gallery.drain(start + FRAME * 2)).is_some(),
		"animations play by default"
	);

	let changed = bridge::change_setting(&mut settings, "media.animation", &serde_json::json!("never"));
	assert_eq!(changed, Some(bridge::SettingKey::Animation));
	assert_eq!(
		changed.map(bridge::SettingKey::applies),
		Some(bridge::Applies::Now)
	);
	images.gallery.set_playback(bridge::media_playback(&settings));
	images.gallery.drain(start + FRAME * 3);
	assert!(images.gallery.drain(start + FRAME * 10).is_empty(), "never plays");

	bridge::change_setting(&mut settings, "media.animation", &serde_json::json!("always"));
	images.gallery.set_playback(bridge::media_playback(&settings));
	let changed = bridge::change_setting(
		&mut settings,
		"accessibility.reduced_motion",
		&serde_json::Value::Bool(true),
	);
	assert_eq!(changed, Some(bridge::SettingKey::ReducedMotion));
	assert_eq!(
		changed.map(bridge::SettingKey::applies),
		Some(bridge::Applies::Now)
	);
	images
		.gallery
		.set_reduced_motion(settings.accessibility.reduced_motion);
	let reset = images.gallery.drain(start + FRAME * 11);
	assert!(
		frame_at(&reset).is_none_or(|frame| frame == first),
		"reduced motion rests on the first frame"
	);
	assert!(images.gallery.drain(start + FRAME * 20).is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_original_resolution_setting_fetches_the_original_attachment() {
	let mut running = Running::start("media-setting-quality").await;
	let images = Images::new(&running, media::DEFAULT_BUDGET_BYTES);
	let channel_id = open_first_channel(&mut running).await;
	let (message_id, attachment) =
		posted_attachment(&mut running, channel_id, "setting.png", Signature::Fresh).await;
	let mut settings = store::Settings::default();
	let request = |settings: &store::Settings| {
		bridge::attachment_request(
			&attachment,
			channel_id,
			message_id,
			(400., 300.),
			1.,
			bridge::media_quality(settings),
		)
		.expect("a png is an image")
	};
	images.ready(request(&settings)).await;
	let changed = bridge::change_setting(
		&mut settings,
		"media.original_resolution",
		&serde_json::Value::Bool(true),
	);
	assert_eq!(changed, Some(bridge::SettingKey::OriginalResolution));
	assert_eq!(
		changed.map(bridge::SettingKey::applies),
		Some(bridge::Applies::Now)
	);
	images.ready(request(&settings)).await;
	let fetched = media_requests(&running, "/cdn/attachments/");
	assert_eq!(fetched.len(), 2);
	assert_eq!(fetched[0].param("width"), Some("400"));
	assert_eq!(fetched[1].param("width"), None, "the original has no proxy size");
	assert_eq!(fetched[1].param("format"), None);
	running.stop().await;
}
