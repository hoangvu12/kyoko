use std::time::Duration;

use mockcord::{Recording, Reply};
use protocol::{ChannelType, Snowflake};
use session::Command;
use state::State;

use crate::harness::Running;

async fn started(name: &'static str) -> Running {
	let running = Running::start(name).await;
	running.connected().await;
	running
}

fn conversation(running: &Running, kind: ChannelType) -> Snowflake {
	let state = running.session().state().read();
	state
		.dm_rows()
		.into_iter()
		.filter_map(|row| state.channel(row.channel_id))
		.find(|channel| channel.kind == kind)
		.map(|channel| channel.id)
		.expect("a fixture conversation of that kind")
}

fn closes(recording: &Recording, channel_id: Snowflake) -> usize {
	recording
		.requests_to("DELETE", &format!("/channels/{channel_id}"))
		.count()
}

fn listed(state: &State, channel_id: Snowflake) -> bool {
	state.dm_rows().iter().any(|row| row.channel_id == channel_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_a_conversation_deletes_the_channel_once_and_drops_it() {
	let running = started("dms-close").await;
	for kind in [ChannelType::Dm, ChannelType::GroupDm] {
		let channel_id = conversation(&running, kind);
		running.session().send(Command::CloseDm { channel_id });
		running
			.until("the conversation to leave the list", |state| {
				!listed(state, channel_id)
			})
			.await;
		tokio::time::sleep(Duration::from_millis(300)).await;
		assert_eq!(closes(&running.mock().recording(), channel_id), 1);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_close_keeps_the_conversation_and_reports_it() {
	let running = started("dms-close-refused").await;
	let channel_id = conversation(&running, ChannelType::Dm);
	running.mock().respond_once(
		"DELETE",
		&format!("/channels/{channel_id}"),
		Reply::error(403, 50001, "Missing Access"),
	);
	running.session().send(Command::CloseDm { channel_id });
	running
		.until("the refusal to be reported", |state| {
			state.last_rejection().is_some()
		})
		.await;
	assert!(listed(&running.session().state().read(), channel_id));
	assert_eq!(closes(&running.mock().recording(), channel_id), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messaging_a_user_opens_the_conversation_and_sends_through_the_outbox() {
	let running = started("dms-message-user").await;
	let user_id = Snowflake(
		running.fixtures().guilds[0].members[1]
			.user
			.as_ref()
			.expect("a member")
			.id
			.0,
	);
	running.session().send(Command::MessageUser {
		user_id,
		content: "hello from the composer".to_owned(),
	});
	let body = running
		.until_wire("the message in the conversation", |recording| {
			recording
				.requests
				.iter()
				.filter(|request| request.method == "POST" && request.path.ends_with("/messages"))
				.filter_map(|request| request.json())
				.find(|body| body["content"] == "hello from the composer")
		})
		.await;
	assert!(body["nonce"].is_string(), "sent through the outbox");
	let recording = running.mock().recording();
	let opened: Vec<_> = recording.requests_to("POST", "/users/@me/channels").collect();
	assert_eq!(opened.len(), 1);
	assert_eq!(
		opened[0].json(),
		Some(serde_json::json!({ "recipients": [user_id.to_string()] }))
	);
	let selected = running
		.session()
		.state()
		.read()
		.selected()
		.expect("the conversation is open");
	assert!(selected.guild_id.is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_conversation_sends_nothing_and_says_why() {
	let running = started("dms-message-refused").await;
	running.mock().respond_once(
		"POST",
		"/users/@me/channels",
		Reply::error(403, 50007, "Cannot send messages to this user"),
	);
	running.session().send(Command::MessageUser {
		user_id: Snowflake(
			running.fixtures().guilds[0].members[1]
				.user
				.as_ref()
				.expect("a member")
				.id
				.0,
		),
		content: "hi".to_owned(),
	});
	running
		.until("the refusal to be reported", |state| {
			state.last_rejection().is_some()
		})
		.await;
	tokio::time::sleep(Duration::from_millis(300)).await;
	assert!(
		!running
			.mock()
			.recording()
			.requests
			.iter()
			.any(|request| request.method == "POST" && request.path.ends_with("/messages"))
	);
	running.stop().await;
}
