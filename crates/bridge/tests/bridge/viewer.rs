use bridge::{ChannelKind, LightboxImage, Viewing, Zoom};
use protocol::Snowflake;

use crate::harness::Running;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_viewer_steps_through_a_messages_images_in_grid_order() {
	let mut running = Running::start("viewer-steps").await;
	let channel_id = opened(&mut running).await;
	let message_id = running
		.server
		.post_images(
			channel_id,
			&[
				("first.png", (1920, 1080)),
				("second.png", (1080, 1920)),
				("third.png", (1000, 1000)),
			],
		)
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
		.expect("the message is in the timeline")
		.clone();
	let second = LightboxImage::attachment(&entry.message.attachments[1], channel_id, message_id)
		.expect("a png is an image");

	let viewing = Viewing::of(&entry, second);
	let names: Vec<&str> = viewing.images.iter().map(|image| image.name.as_str()).collect();
	assert_eq!(names, ["first.png", "second.png", "third.png"]);
	assert_eq!(viewing.index, 1, "the viewer opens on the clicked image");
	assert_eq!(viewing.message, Some((channel_id, message_id)));
	let author = viewing.author.as_ref().expect("the author shows");
	assert_eq!(author.name, entry.author);
	assert_eq!(author.sent_at, entry.sent_at);

	let next = viewing.stepped(1).expect("a next image");
	assert_eq!(next.current().name, "third.png");
	let wrapped = next.stepped(1).expect("the last image wraps");
	assert_eq!(wrapped.current().name, "first.png");
	assert_eq!(wrapped.stepped(-1).expect("back").current().name, "third.png");
	assert!(
		Viewing::alone(viewing.current().clone()).stepped(1).is_none(),
		"a lone image has nowhere to step"
	);
	running.stop().await;
}

#[test]
fn zoom_keeps_the_pointed_spot_still_and_the_image_on_screen() {
	let fitted = (400., 300.);
	let room = (1000., 800.);
	let toggled = Zoom::FIT.toggled(fitted, room);
	assert_eq!(toggled.scale, bridge::VIEWER_ZOOM_STEP);
	assert_eq!(toggled.pan, (0., 0.));
	assert_eq!(
		toggled.toggled(fitted, room),
		Zoom::FIT,
		"the zoom button toggles back"
	);

	let about = (100., 50.);
	let narrow = (500., 400.);
	let wheeled = Zoom::FIT.wheeled(-400., about, fitted, narrow);
	assert!(wheeled.zoomed(), "a wheel up zooms in");
	let spot = |zoom: Zoom| {
		(
			(about.0 - zoom.pan.0) / zoom.scale,
			(about.1 - zoom.pan.1) / zoom.scale,
		)
	};
	let (before, after) = (spot(Zoom::FIT), spot(wheeled));
	assert!((before.0 - after.0).abs() < 0.01 && (before.1 - after.1).abs() < 0.01);
	assert_eq!(
		wheeled.wheeled(100_000., about, fitted, narrow),
		Zoom::FIT,
		"zooming out stops at the fitted size, centred"
	);
	assert_eq!(
		Zoom::FIT.wheeled(-100_000., about, fitted, room).scale,
		bridge::VIEWER_MAX_ZOOM
	);

	let big = Zoom::FIT.scaled_to(4., (0., 0.), fitted, room);
	let reach = ((1600. - 1000.) / 2., (1200. - 800.) / 2.);
	assert_eq!(
		big.panned((10_000., -10_000.), fitted, room).pan,
		(reach.0, -reach.1)
	);
	assert_eq!(
		Zoom::FIT.panned((50., 50.), fitted, room).pan,
		(0., 0.),
		"a fitted image does not pan"
	);
}
