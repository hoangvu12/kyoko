use std::time::Duration;

use mockcord::Recording;
use protocol::{ChannelType, Opcode, Snowflake};
use serde_json::{Value, json};
use session::Command;

use crate::harness::{Options, Running};

const SETTLE: Duration = Duration::from_millis(1500);
const UNKNOWN_MEMBER: u64 = 880_000_000_000_000_001;
const IMPOSSIBLE_MENTIONS: [&str; 3] = ["0", "18446744073709551615", "9223372036854775808"];
const LIVE_ID_OFFSET: u64 = 1 << 30;

fn mention_message(running: &Running, channel_id: Snowflake, guild_id: Snowflake, content: &str) -> Value {
	let newest = running.fixtures().messages[&channel_id.get()]
		.keys()
		.next_back()
		.copied()
		.unwrap_or_default();
	json!({
		"id": (newest + LIVE_ID_OFFSET).to_string(),
		"channel_id": channel_id.to_string(),
		"guild_id": guild_id.to_string(),
		"author": running.fixtures().current_user,
		"content": content,
		"type": 0,
		"timestamp": "2026-01-01T00:00:00Z",
		"mentions": [],
	})
}

fn requested_ids(recording: &Recording) -> Vec<String> {
	recording
		.frames_with(Opcode::RequestGuildMembers)
		.flat_map(|frame| frame.data["user_ids"].as_array().cloned().unwrap_or_default())
		.filter_map(|id| id.as_str().map(str::to_owned))
		.collect()
}

fn subscribed_guilds(recording: &Recording) -> Vec<String> {
	recording
		.frames_with(Opcode::GuildSubscriptionsBulk)
		.flat_map(|frame| {
			frame.data["subscriptions"]
				.as_object()
				.map(|guilds| guilds.keys().cloned().collect::<Vec<_>>())
				.unwrap_or_default()
		})
		.collect()
}

fn text_channel_of(running: &Running, guild_id: Snowflake) -> Snowflake {
	running
		.session()
		.state()
		.read()
		.guild(guild_id)
		.and_then(|guild| {
			guild
				.channels
				.values()
				.find(|channel| channel.kind == ChannelType::GuildText)
				.map(|channel| channel.id)
		})
		.expect("a text channel in the guild")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mentions_of_impossible_ids_never_reach_a_member_request() {
	let running = Running::start("payloads-mentions").await;
	let channel_id = running.open_text_channel().await;
	let guild_id = running
		.session()
		.state()
		.read()
		.channel_guild(channel_id)
		.expect("a guild channel");
	let mut content = IMPOSSIBLE_MENTIONS
		.iter()
		.map(|id| format!("<@{id}> "))
		.collect::<String>();
	content.push_str(&format!("<@{UNKNOWN_MEMBER}>"));
	running.mock().dispatch(
		"MESSAGE_CREATE",
		&mention_message(&running, channel_id, guild_id, &content),
	);
	let unknown = UNKNOWN_MEMBER.to_string();
	running
		.until_wire("a member request for the real mention", |recording| {
			requested_ids(recording).contains(&unknown).then_some(())
		})
		.await;
	tokio::time::sleep(SETTLE).await;
	let requested = requested_ids(&running.mock().recording());
	for impossible in IMPOSSIBLE_MENTIONS {
		assert!(
			!requested.iter().any(|id| id == impossible),
			"{impossible} was requested"
		);
	}
	assert_eq!(running.session().reconnects(), 0, "the gateway was closed");
	assert!(running.session().state().read().session().connected);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pending_membership_asks_for_no_members_or_member_list() {
	let options = Options::new("payloads-pending").prepare(|mock| {
		mock.mark_first_guild_pending();
	});
	let running = Running::start_with(options).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.until("the membership to read as pending", |state| {
			state.is_membership_pending(guild_id)
		})
		.await;
	let channel_id = text_channel_of(&running, guild_id);
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the channel history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
	running.session().send(Command::SearchMembers {
		guild_id,
		query: "mo".to_owned(),
	});
	running.session().send(Command::RequestMembers {
		guild_id,
		user_ids: vec![Snowflake(UNKNOWN_MEMBER)],
	});
	running.mock().dispatch(
		"MESSAGE_CREATE",
		&mention_message(&running, channel_id, guild_id, &format!("<@{UNKNOWN_MEMBER}>")),
	);
	tokio::time::sleep(SETTLE).await;
	let recording = running.mock().recording();
	let guild = guild_id.to_string();
	assert!(!subscribed_guilds(&recording).contains(&guild));
	assert!(
		recording
			.frames_with(Opcode::RequestGuildMembers)
			.all(|frame| !frame.data["guild_id"].to_string().contains(&guild))
	);
	assert_eq!(running.session().reconnects(), 0, "the gateway was closed");
	running.stop().await;
}
