use std::time::Duration;

use plugins::Limits;

use crate::support::{approve, scratch, start, write_plugin};

fn counter(id: &str) -> String {
	format!(
		r#"
local created = 0
return {{
	on_event = function(event)
		if event.type == "message_create" and event.message.content == "ping" then
			created += 1
		end
	end,
	before_send = function(message)
		if message.content == "ping" then
			return nil
		end
		return message.content .. " {id}=" .. created
	end,
}}
"#
	)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn events_reach_only_plugins_allowed_to_read_them() {
	let root = scratch("events");
	write_plugin(&root, "blind", &["hook:send"], "", &counter("blind"));
	write_plugin(
		&root,
		"reader",
		&["hook:send", "read:messages"],
		"",
		&counter("reader"),
	);
	let lab = start("events", root, Limits::default(), false).await;
	approve(&lab.host, "blind");
	approve(&lab.host, "reader");
	let channel_id = lab.running.open_text_channel().await;
	lab.running.send_and_wait(channel_id, "ping", "ping").await;
	tokio::time::sleep(Duration::from_millis(200)).await;
	lab.running
		.send_and_wait(channel_id, "count", "count blind=0 reader=1")
		.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_plugin_hears_who_signed_in() {
	let root = scratch("ready");
	write_plugin(
		&root,
		"greeter",
		&["hook:send"],
		"",
		r#"
local signed_in = "nobody"
return {
	on_event = function(event)
		if event.type == "ready" then
			signed_in = event.user_id
		end
	end,
	before_send = function(message)
		return message.content .. " as " .. signed_in
	end,
}
"#,
	);
	let mut lab = start("ready", root, Limits::default(), false).await;
	approve(&lab.host, "greeter");
	lab.running.restart().await;
	let channel_id = lab.running.open_text_channel().await;
	let me = lab
		.running
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("signed in");
	lab.running
		.send_and_wait(channel_id, "hello", &format!("hello as {me}"))
		.await;
	lab.stop().await;
}
