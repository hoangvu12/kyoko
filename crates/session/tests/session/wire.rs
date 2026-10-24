use std::collections::BTreeSet;
use std::time::Duration;

use mockcord::Reply;
use protocol::{Opcode, Snowflake};
use serde_json::json;

use crate::harness::{LOCALE, Running};

const MESSAGES_ROUTE: &str = "/channels/{id}/messages";
const RETRY_AFTER: f64 = 0.25;
const LATE_CHANNEL_ID: u64 = 990_000_000_000_000_001;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_reaches_ready_and_lists_every_guild() {
	let running = Running::start("login").await;
	let expected: BTreeSet<Snowflake> = running
		.fixtures()
		.guilds
		.iter()
		.map(|guild| Snowflake(guild.id))
		.collect();
	running
		.until("every fixture guild in the snapshot", |state| {
			state.session().connected
				&& state.guilds().map(|guild| guild.id).collect::<BTreeSet<_>>() == expected
		})
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(
			state.current_user().map(|user| user.id),
			Some(running.fixtures().current_user.id)
		);
		for fixture in &running.fixtures().guilds {
			let guild = state.guild(Snowflake(fixture.id)).expect("fixture guild");
			assert_eq!(Some(&guild.name), fixture.properties.name.as_ref());
		}
	}
	let recording = running.mock().recording();
	let identifies: Vec<_> = recording.frames_with(Opcode::Identify).collect();
	assert_eq!(identifies.len(), 1);
	assert_eq!(recording.frames_with(Opcode::Resume).count(), 0);
	let identify = &identifies[0].data;
	assert_eq!(identify["token"].as_str(), Some(running.token()));
	assert!(identify["capabilities"].is_u64());
	assert!(identify["properties"]["os"].is_string());
	assert!(identify["properties"]["browser"].is_string());
	let user_agent = identify["properties"]["browser_user_agent"]
		.as_str()
		.expect("identify carries the browser user agent");
	let connection = recording
		.connections
		.iter()
		.find(|connection| connection.id == identifies[0].connection)
		.expect("the identify connection was recorded");
	assert_eq!(connection.header("user-agent"), Some(user_agent));
	assert_eq!(connection.header("origin"), Some("https://discord.com"));
	assert!(
		connection
			.query
			.as_deref()
			.is_some_and(|query| query.contains("encoding=json"))
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sent_message_is_observed_on_the_wire() {
	let running = Running::start("send").await;
	let channel_id = running.open_text_channel().await;
	running
		.send_and_wait(channel_id, "hello wire", "hello wire")
		.await;
	let path = format!("/channels/{channel_id}/messages");
	let sends: Vec<_> = running
		.mock()
		.recording()
		.requests_to("POST", &path)
		.cloned()
		.collect();
	assert_eq!(sends.len(), 1);
	let send = &sends[0];
	assert!((200..300).contains(&send.status));
	assert!(!send.injected);
	let body = send.json().expect("a JSON body");
	assert_eq!(body["content"].as_str(), Some("hello wire"));
	assert!(body["nonce"].is_string());
	assert_eq!(send.header("authorization"), Some(running.token()));
	assert_eq!(send.header("content-type"), Some("application/json"));
	assert_eq!(send.header("x-discord-locale"), Some(LOCALE));
	for required in ["user-agent", "x-super-properties", "accept-language"] {
		assert!(send.header(required).is_some(), "{required} header missing");
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_injected_rate_limit_is_waited_out_before_the_retry() {
	let running = Running::start("rate-limit").await;
	let channel_id = running.open_text_channel().await;
	running
		.mock()
		.respond_once("POST", MESSAGES_ROUTE, Reply::rate_limited(RETRY_AFTER, false));
	running
		.send_and_wait(channel_id, "after the limit", "after the limit")
		.await;
	let path = format!("/channels/{channel_id}/messages");
	let sends: Vec<_> = running
		.mock()
		.recording()
		.requests_to("POST", &path)
		.cloned()
		.collect();
	assert_eq!(sends.len(), 2);
	assert_eq!(sends[0].status, 429);
	assert!(sends[0].injected);
	assert!((200..300).contains(&sends[1].status));
	assert_eq!(sends[0].json(), sends[1].json());
	assert!(sends[1].at.duration_since(sends[0].at) >= Duration::from_secs_f64(RETRY_AFTER));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dropped_gateway_resumes_and_receives_late_dispatches() {
	let running = Running::start("resume").await;
	running.connected().await;
	let first = running.identified_connection().await;
	running.mock().close_gateway(4000);
	let resume = running
		.until_wire("a RESUME on a new connection", |recording| {
			recording
				.frames_with(Opcode::Resume)
				.find(|frame| frame.connection != first)
				.cloned()
		})
		.await;
	assert_eq!(resume.data["token"].as_str(), Some(running.token()));
	assert!(resume.data["session_id"].is_string());
	assert!(resume.data["seq"].as_u64().is_some_and(|seq| seq > 0));
	let session = running.session();
	running
		.until("the session to come back", |state| {
			session.reconnects() >= 1 && state.session().connected
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(recording.frames_with(Opcode::Identify).count(), 1);
	assert!(
		recording
			.closes
			.iter()
			.all(|close| close.connection != resume.connection)
	);
	let guild_id = running.fixtures().guilds[0].id;
	running.mock().dispatch(
		"CHANNEL_CREATE",
		&json!({
			"id": LATE_CHANNEL_ID.to_string(),
			"type": 0,
			"guild_id": guild_id.to_string(),
			"name": "late-channel",
			"position": 99,
			"permission_overwrites": [],
		}),
	);
	running
		.until("the late channel to appear", |state| {
			state
				.guild(Snowflake(guild_id))
				.is_some_and(|guild| guild.channels.contains_key(&Snowflake(LATE_CHANNEL_ID)))
		})
		.await;
	running.stop().await;
}
