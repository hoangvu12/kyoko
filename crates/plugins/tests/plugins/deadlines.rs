use std::time::{Duration, Instant};

use plugins::{Limits, Status};

use crate::support::{approve, scratch, start, write_plugin};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadline_overruns_suspend_the_plugin_without_holding_up_sends() {
	let root = scratch("overrun");
	write_plugin(
		&root,
		"spinner",
		&["hook:send"],
		"",
		"return { before_send = function() while true do end end }",
	);
	write_plugin(
		&root,
		"trapper",
		&["hook:send"],
		"",
		"return { before_send = function() while true do pcall(function() while true do end end) end end }",
	);
	let limits = Limits {
		send_deadline: Duration::from_millis(100),
		strikes: 2,
		backoff: Duration::from_secs(60),
		..Limits::default()
	};
	let lab = start("overrun", root, limits, false).await;
	approve(&lab.host, "spinner");
	approve(&lab.host, "trapper");
	let channel_id = lab.running.open_text_channel().await;
	for text in ["first", "second"] {
		let began = Instant::now();
		lab.running.send_and_wait(channel_id, text, text).await;
		assert!(
			began.elapsed() < Duration::from_secs(3),
			"an overrunning hook is cut off at its deadline"
		);
	}
	for id in ["spinner", "trapper"] {
		assert!(
			matches!(lab.host.status(id), Some(Status::Suspended { .. })),
			"{id} is suspended after two overruns: {:?}",
			lab.host.status(id)
		);
	}
	let notices = lab.host.take_notices();
	assert!(
		notices
			.iter()
			.any(|notice| notice.plugin == "spinner" && notice.text.contains("ran past its time limit")),
		"the user is told why: {notices:?}"
	);
	let began = Instant::now();
	lab.running.send_and_wait(channel_id, "third", "third").await;
	assert!(
		began.elapsed() < Duration::from_secs(2),
		"a suspended plugin is not called"
	);
	lab.running
		.until("the session to stay connected", |state| state.session().connected)
		.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_plugin_that_keeps_failing_comes_back_after_its_backoff_then_turns_off() {
	let root = scratch("failing");
	write_plugin(
		&root,
		"broken",
		&["hook:send"],
		"",
		"return { before_send = function() error(\"boom\") end }",
	);
	let limits = Limits {
		strikes: 1,
		backoff: Duration::from_millis(200),
		suspensions: 2,
		..Limits::default()
	};
	let lab = start("failing", root, limits, false).await;
	approve(&lab.host, "broken");
	let channel_id = lab.running.open_text_channel().await;
	lab.running.send_and_wait(channel_id, "a", "a").await;
	assert!(matches!(
		lab.host.status("broken"),
		Some(Status::Suspended { .. })
	));
	tokio::time::sleep(Duration::from_millis(300)).await;
	lab.running.send_and_wait(channel_id, "b", "b").await;
	assert_eq!(lab.host.status("broken"), Some(Status::Stopped));
	let plugin = lab
		.host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == "broken")
		.expect("still listed");
	assert!(!plugin.enabled, "turned off until the user turns it back on");
	assert!(plugin.notice.is_some_and(|notice| notice.contains("turned off")));
	lab.host.enable("broken").expect("the user can turn it back on");
	assert_eq!(lab.host.status("broken"), Some(Status::Running));
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_plugin_that_does_not_load_is_reported_and_the_rest_keep_working() {
	let root = scratch("unloadable");
	write_plugin(
		&root,
		"garbled",
		&["hook:send"],
		"",
		"return { before_send = function( end }",
	);
	write_plugin(
		&root,
		"working",
		&["hook:send"],
		"",
		"return { before_send = function(message) return message.content .. \"!\" end }",
	);
	let lab = start("unloadable", root, Limits::default(), false).await;
	approve(&lab.host, "garbled");
	approve(&lab.host, "working");
	assert!(matches!(lab.host.status("garbled"), Some(Status::Failed(_))));
	let channel_id = lab.running.open_text_channel().await;
	lab.running.send_and_wait(channel_id, "still", "still!").await;
	lab.stop().await;
}
