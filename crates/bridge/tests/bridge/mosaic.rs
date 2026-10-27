use std::sync::Arc;
use std::time::Duration;

use bridge::{ChannelKind, MOSAIC_GAP, MOSAIC_WIDTH, Mosaic, Tile};
use media::cdn::Quality;
use media::{Gallery, ImageName, Limits, MediaService, Status};
use protocol::Snowflake;
use protocol::fingerprint::Fingerprint;
use tokio::sync::Notify;
use tokio::time::timeout;

use crate::harness::Running;

const WAIT: Duration = Duration::from_secs(15);
const SCALE: f64 = 1.5;

async fn opened(running: &mut Running) -> Snowflake {
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("the first guild has a text channel");
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	channel_id
}

async fn posted(running: &mut Running, channel_id: Snowflake, images: &[(&str, (u32, u32))]) -> Mosaic {
	let message_id = running
		.server
		.post_images(channel_id, images)
		.expect("mockcord posts the images");
	running
		.until("the images to reach the timeline", |view| {
			view.timeline.messages().any(|entry| entry.id == message_id)
		})
		.await;
	let entry = running
		.view
		.timeline
		.messages()
		.find(|entry| entry.id == message_id)
		.expect("the message is in the timeline");
	bridge::image_mosaic(&bridge::mosaic_images(&entry.message.attachments))
}

fn close(left: f32, right: f32) -> bool {
	(left - right).abs() < 0.01
}

fn spans_the_width(tiles: &[Tile]) -> bool {
	let first = tiles.first().expect("a row has tiles");
	let last = tiles.last().expect("a row has tiles");
	close(first.x, 0.) && close(last.x + last.width, MOSAIC_WIDTH)
}

fn rows_of(mosaic: &Mosaic) -> Vec<Vec<Tile>> {
	let mut rows: Vec<Vec<Tile>> = Vec::new();
	for tile in &mosaic.tiles {
		match rows.last_mut() {
			Some(row) if close(row[0].y, tile.y) => row.push(*tile),
			_ => rows.push(vec![*tile]),
		}
	}
	rows
}

async fn settled(gallery: &Gallery, woken: &Notify, name: &ImageName) -> Status {
	let deadline = tokio::time::Instant::now() + WAIT;
	loop {
		let wake = woken.notified();
		if let Some(status @ (Status::Ready { .. } | Status::Failed)) = gallery.status(name) {
			return status;
		}
		if timeout(deadline - tokio::time::Instant::now(), wake)
			.await
			.is_err()
		{
			panic!("timed out waiting for {name}");
		}
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn several_images_in_a_message_share_discords_mosaic() {
	let mut running = Running::start("mosaic-layouts").await;
	let channel_id = opened(&mut running).await;

	let wide = posted(
		&mut running,
		channel_id,
		&[("before.png", (1650, 150)), ("after.png", (1650, 144))],
	)
	.await;
	assert_eq!(wide.tiles.len(), 2);
	assert!(
		wide.tiles.iter().all(|tile| close(tile.width, MOSAIC_WIDTH)),
		"two wide images stack at full width: {wide:?}"
	);
	assert!(close(wide.tiles[0].height, 50.) && close(wide.tiles[1].height, 48.));
	assert!(close(wide.tiles[1].y, 50. + MOSAIC_GAP));

	let mixed = posted(
		&mut running,
		channel_id,
		&[("wide.png", (1280, 720)), ("tall.png", (720, 1280))],
	)
	.await;
	let half = (MOSAIC_WIDTH - MOSAIC_GAP) / 2.;
	assert_eq!(
		rows_of(&mixed).len(),
		1,
		"a wide and a tall image sit side by side"
	);
	assert!(
		mixed
			.tiles
			.iter()
			.all(|tile| close(tile.width, half) && close(tile.height, half))
	);

	let three = posted(
		&mut running,
		channel_id,
		&[
			("a.png", (1920, 1080)),
			("b.png", (1080, 1920)),
			("c.png", (1000, 1000)),
		],
	)
	.await;
	let small = (MOSAIC_WIDTH - 2. * MOSAIC_GAP) / 3.;
	let large = MOSAIC_WIDTH - MOSAIC_GAP - small;
	assert!(close(three.tiles[0].width, large) && close(three.tiles[0].height, large));
	assert!(close(three.tiles[1].x, three.tiles[2].x) && close(three.tiles[1].width, small));
	assert!(close(three.tiles[2].y + three.tiles[2].height, three.height));

	let shapes = [(1600, 900), (900, 1600), (800, 800), (2400, 600)];
	let names = [
		"1.png", "2.png", "3.png", "4.png", "5.png", "6.png", "7.png", "8.png", "9.png", "10.png",
	];
	let expected: [&[usize]; 7] = [
		&[2, 2],
		&[2, 3],
		&[3, 3],
		&[1, 3, 3],
		&[2, 3, 3],
		&[3, 3, 3],
		&[1, 3, 3, 3],
	];
	for (count, rows) in (4..=10).zip(expected) {
		let images: Vec<(&str, (u32, u32))> = names
			.iter()
			.zip(shapes.iter().cycle())
			.take(count)
			.map(|(name, shape)| (*name, *shape))
			.collect();
		let grid = posted(&mut running, channel_id, &images).await;
		let laid: Vec<Vec<Tile>> = rows_of(&grid);
		assert_eq!(
			laid.iter().map(Vec::len).collect::<Vec<_>>(),
			rows,
			"{count} images"
		);
		assert!(
			laid.iter().all(|row| spans_the_width(row)),
			"{count} images fill the width"
		);
		let last = grid.tiles.last().expect("tiles");
		assert!(close(last.y + last.height, grid.height));
	}

	let single = posted(&mut running, channel_id, &[("alone.png", (1000, 1000))]).await;
	assert_eq!(single.tiles.len(), 1);
	assert!(
		close(single.width, 350.) && close(single.height, 350.),
		"one image keeps its shape inside 550 by 350: {single:?}"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_tile_fetches_an_image_that_covers_it() {
	let mut running = Running::start("mosaic-requests").await;
	let channel_id = opened(&mut running).await;
	let message_id = running
		.server
		.post_images(
			channel_id,
			&[("wide.png", (1280, 720)), ("tall.png", (720, 1280))],
		)
		.expect("mockcord posts the images");
	running
		.until("the images to reach the timeline", |view| {
			view.timeline.messages().any(|entry| entry.id == message_id)
		})
		.await;
	let attachments = running
		.view
		.timeline
		.messages()
		.find(|entry| entry.id == message_id)
		.expect("the message is in the timeline")
		.message
		.attachments
		.clone();
	let images = bridge::mosaic_images(&attachments);
	let mosaic = bridge::image_mosaic(&images);

	let fingerprint = Fingerprint::web_client("en-US", 1);
	let fetcher = bridge::media_fetcher(&fingerprint, Some(running.server.address().port()))
		.expect("the fetcher builds");
	let loader = MediaService::new(fetcher, running.scratch.join("cache"), Limits::default());
	let gallery = Gallery::with_budget(
		loader,
		tokio::runtime::Handle::current(),
		media::DEFAULT_BUDGET_BYTES,
	);
	let woken = Arc::new(Notify::new());
	let notify = woken.clone();
	gallery.set_waker(move || notify.notify_one());
	for (attachment, tile) in images.iter().zip(&mosaic.tiles) {
		let cover = bridge::cover_box(attachment.width, attachment.height, tile.size());
		let request = bridge::attachment_request(
			attachment,
			channel_id,
			message_id,
			(f64::from(cover.0), f64::from(cover.1)),
			SCALE,
			Quality::Fitted,
		)
		.expect("a png is an image");
		let name = gallery.request(request);
		assert!(matches!(
			settled(&gallery, &woken, &name).await,
			Status::Ready { .. }
		));
	}
	let fetched: Vec<(u32, u32)> = running
		.server
		.control()
		.recording()
		.media_to("/cdn/attachments/")
		.map(|request| {
			let side = |key: &str| {
				request
					.param(key)
					.and_then(|value| value.parse().ok())
					.unwrap_or(0)
			};
			(side("width"), side("height"))
		})
		.collect();
	let side = ((MOSAIC_WIDTH - MOSAIC_GAP) / 2. * SCALE as f32).ceil() as u32;
	assert_eq!(fetched.len(), 2, "{fetched:?}");
	assert!(
		fetched
			.iter()
			.all(|(width, height)| (*width).min(*height) + 1 >= side),
		"each tile of {side} device pixels is covered, not fitted inside: {fetched:?}"
	);
	assert!(fetched[0].0 > fetched[0].1 && fetched[1].1 > fetched[1].0);
	running.stop().await;
}
