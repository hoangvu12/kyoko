use plugins::{EnableError, Limits, Permission, Status};
use serde_json::json;

use crate::support::{scratch, start};

const ID: &str = "clear-urls";

fn asked() -> Vec<Permission> {
	vec![Permission::parse("hook:send"), Permission::parse("hook:edit")]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clear_urls_strips_tracking_parameters_once_approved() {
	let lab = start("clear-urls", scratch("clear-urls"), Limits::default(), true).await;
	let channel_id = lab.running.open_text_channel().await;
	assert_eq!(lab.host.status(ID), Some(Status::Disabled));
	assert_eq!(
		lab.host.enable(ID),
		Err(EnableError::NeedsApproval(asked())),
		"enabling asks for approval first"
	);
	let untouched = "before https://example.com/a?utm_source=news&id=3";
	lab.running.send_and_wait(channel_id, untouched, untouched).await;

	lab.host.approve(ID, &asked()).expect("approved");
	assert_eq!(lab.host.status(ID), Some(Status::Running));
	let message_id = lab
		.running
		.send_and_wait(
			channel_id,
			"see https://www.youtube.com/watch?v=abc&si=track&utm_medium=social, and `https://x.com/?utm_source=kept`",
			"see https://www.youtube.com/watch?v=abc, and `https://x.com/?utm_source=kept`",
		)
		.await;
	assert_eq!(
		lab.running.session().state().read().altered_by(message_id),
		Some("ClearURLs")
	);

	lab.edit_and_wait(
		channel_id,
		message_id,
		"https://www.amazon.co.uk/dp/B01/ref=sr_1_2?tag=kyoko-21&keywords=lamp&fbclid=abc",
		"https://www.amazon.co.uk/dp/B01?keywords=lamp",
	)
	.await;

	lab.host
		.set_setting(ID, "referrals", &json!(false))
		.expect("the setting is accepted");
	lab.edit_and_wait(
		channel_id,
		message_id,
		"https://amazon.com/dp/B01?tag=kyoko-21&gclid=x",
		"https://amazon.com/dp/B01?tag=kyoko-21",
	)
	.await;
	lab.stop().await;
}
