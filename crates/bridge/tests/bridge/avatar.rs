use media::{Limits, MediaFetcher, MediaService};

use crate::harness::Running;

const WEBP_MAGIC: &[u8] = b"WEBP";

fn media_for(running: &Running) -> MediaService {
	let fetcher = MediaFetcher::builder()
		.allow_loopback_http(running.server.address().port())
		.build()
		.expect("the fetcher builds");
	MediaService::new(fetcher, running.scratch.join("media"), Limits::default())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_signed_in_users_webp_avatar_decodes_at_device_pixels() {
	let running = Running::start("avatar").await;
	let user = running
		.session()
		.state()
		.read()
		.current_user()
		.cloned()
		.expect("READY names the signed-in user");
	let pixels = bridge::device_pixels(32., 1.5);
	assert_eq!(pixels, 64);
	let url =
		bridge::avatar_url(&running.server.cdn_url(), &user, pixels).expect("the mock user has an avatar");
	assert!(url.contains(".webp?size=64"));

	let media = media_for(&running);
	let image = bridge::load_avatar(&media, &url, pixels)
		.await
		.expect("the avatar loads");
	assert_eq!((image.width, image.height), (64, 64));
	assert_eq!(image.rgba.len(), 64 * 64 * 4);
	assert!(image.rgba.chunks(4).all(|pixel| pixel[3] == u8::MAX));

	let raw = media
		.disk()
		.get(&url)
		.expect("the disk cache reads")
		.expect("the avatar was cached");
	assert_eq!(&raw[..4], b"RIFF");
	assert_eq!(&raw[8..12], WEBP_MAGIC);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_avatar_is_an_error_not_a_panic() {
	let running = Running::start("avatar-missing").await;
	let media = media_for(&running);
	let url = format!("{}/avatars/1/nothing.png", running.server.cdn_url());
	assert!(bridge::load_avatar(&media, &url, 64).await.is_err());
	running.stop().await;
}
