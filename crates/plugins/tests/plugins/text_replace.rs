use std::time::{Duration, Instant};

use plugins::{Limits, PluginHost, Status};
use protocol::Snowflake;
use serde_json::{Value, json};

use crate::support::{Lab, approve, scratch, start};

const ID: &str = "text-replace";
const NAME: &str = "TextReplace";

fn rules(host: &PluginHost, rules: Value) {
	host.set_setting(ID, "rules", &rules)
		.expect("the rules are accepted");
}

fn sent_body(lab: &Lab, channel_id: Snowflake) -> Value {
	let path = format!("/channels/{channel_id}/messages");
	lab.running
		.mock()
		.recording()
		.requests_to("POST", &path)
		.filter_map(|request| request.json())
		.last()
		.expect("a message went out")
}

fn edit_body(lab: &Lab, channel_id: Snowflake, message_id: Snowflake) -> Value {
	let path = format!("/channels/{channel_id}/messages/{message_id}");
	lab.running
		.mock()
		.recording()
		.requests_to("PATCH", &path)
		.filter_map(|request| request.json())
		.last()
		.expect("an edit went out")
}

async fn sends(lab: &Lab, channel_id: Snowflake, typed: &str, wire: &str) -> Snowflake {
	let message_id = lab.running.send_and_wait(channel_id, typed, wire).await;
	assert_eq!(sent_body(lab, channel_id)["content"], json!(wire));
	message_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn text_replace_rewrites_sends_and_edits_in_rule_order() {
	let lab = start(
		"text-replace-order",
		scratch("text-replace-order"),
		Limits::default(),
		true,
	)
	.await;
	rules(
		&lab.host,
		json!([
			{ "find": "hello", "replace": "hi" },
			{ "find": "hi", "replace": "hey there" },
			{ "find": "(%d+)km", "replace": "%1 kilometres", "pattern": true },
			{ "find": "%d+km", "replace": "never applied as a pattern" },
		]),
	);
	approve(&lab.host, ID);
	assert_eq!(lab.host.status(ID), Some(Status::Running));
	let channel_id = lab.running.open_text_channel().await;

	let message_id = sends(
		&lab,
		channel_id,
		"hello, I ran 5km",
		"hey there, I ran 5 kilometres",
	)
	.await;
	assert_eq!(
		lab.running.session().state().read().altered_by(message_id),
		Some(NAME)
	);

	lab.edit_and_wait(
		channel_id,
		message_id,
		"hello again, 12km",
		"hey there again, 12 kilometres",
	)
	.await;
	assert_eq!(
		edit_body(&lab, channel_id, message_id),
		json!({ "content": "hey there again, 12 kilometres" })
	);

	rules(
		&lab.host,
		json!([
			{ "find": "hi", "replace": "hey there" },
			{ "find": "hello", "replace": "hi" },
		]),
	);
	sends(&lab, channel_id, "hello", "hi").await;
	lab.edit_and_wait(channel_id, message_id, "plain words", "plain words")
		.await;
	assert_eq!(
		edit_body(&lab, channel_id, message_id),
		json!({ "content": "plain words" })
	);
	lab.running
		.until("the unaltered edit to clear the mark", |state| {
			state.altered_by(message_id).is_none()
		})
		.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rule_with_a_condition_applies_only_when_the_message_includes_it() {
	let lab = start(
		"text-replace-only-if",
		scratch("text-replace-only-if"),
		Limits::default(),
		true,
	)
	.await;
	rules(
		&lab.host,
		json!([
			{ "find": "brb", "replace": "be right back", "only_if": "!" },
			{ "find": "gg", "replace": "good game", "only_if": "" },
		]),
	);
	approve(&lab.host, ID);
	let channel_id = lab.running.open_text_channel().await;
	sends(&lab, channel_id, "brb gg", "brb good game").await;
	sends(&lab, channel_id, "brb!", "be right back!").await;
	let message_id = sends(&lab, channel_id, "ok", "ok").await;
	lab.edit_and_wait(channel_id, message_id, "brb", "brb").await;
	lab.edit_and_wait(channel_id, message_id, "brb now!", "be right back now!")
		.await;
	assert_eq!(
		edit_body(&lab, channel_id, message_id),
		json!({ "content": "be right back now!" })
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_broken_rule_fails_alone_and_never_breaks_the_send() {
	let lab = start(
		"text-replace-broken",
		scratch("text-replace-broken"),
		Limits::default(),
		true,
	)
	.await;
	rules(
		&lab.host,
		json!([
			{ "find": "[unclosed", "replace": "x", "pattern": true },
			{ "find": "cat", "replace": "%q", "pattern": true },
			{ "find": "cat", "replace": "dog" },
			{ "find": "", "replace": "everywhere" },
		]),
	);
	approve(&lab.host, ID);
	let channel_id = lab.running.open_text_channel().await;
	sends(&lab, channel_id, "a cat and [unclosed", "a dog and [unclosed").await;
	sends(&lab, channel_id, "a cat again", "a dog again").await;
	assert_eq!(
		lab.host.status(ID),
		Some(Status::Running),
		"a rule that fails is skipped without counting against the plugin"
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_pattern_hits_the_deadline_and_the_message_goes_out_unchanged() {
	let limits = Limits {
		send_deadline: Duration::from_millis(100),
		..Limits::default()
	};
	let lab = start("text-replace-slow", scratch("text-replace-slow"), limits, true).await;
	rules(
		&lab.host,
		json!([
			{ "find": "x", "replace": "y" },
			{ "find": "a*a*a*a*a*a*a*a*c", "replace": "never", "pattern": true },
		]),
	);
	approve(&lab.host, ID);
	let channel_id = lab.running.open_text_channel().await;
	let typed = format!("x {}", "a".repeat(80));
	let started = Instant::now();
	sends(&lab, channel_id, &typed, &typed).await;
	assert!(
		started.elapsed() < Duration::from_secs(5),
		"the send waited {:?} for a runaway pattern",
		started.elapsed()
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn text_replace_changes_nothing_while_it_is_off() {
	let lab = start(
		"text-replace-off",
		scratch("text-replace-off"),
		Limits::default(),
		true,
	)
	.await;
	rules(&lab.host, json!([{ "find": "colour", "replace": "color" }]));
	assert_eq!(lab.host.status(ID), Some(Status::Disabled), "it ships disabled");
	let channel_id = lab.running.open_text_channel().await;
	let message_id = sends(&lab, channel_id, "my colour", "my colour").await;
	lab.edit_and_wait(channel_id, message_id, "your colour", "your colour")
		.await;
	assert_eq!(
		edit_body(&lab, channel_id, message_id),
		json!({ "content": "your colour" })
	);

	approve(&lab.host, ID);
	sends(&lab, channel_id, "my colour", "my color").await;

	lab.host.disable(ID);
	sends(&lab, channel_id, "their colour", "their colour").await;
	lab.edit_and_wait(channel_id, message_id, "our colour", "our colour")
		.await;
	lab.stop().await;
}
