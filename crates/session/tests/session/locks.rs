use std::collections::HashSet;

use chrono::Utc;
use mockcord::{Control, Fixtures};
use protocol::{ChannelType, Snowflake};
use serde_json::{Value, json};
use session::{Command, RiskyReason, RiskyTarget};
use state::State;

use crate::harness::{Options, Running};

const OPEN_DM: &str = "/users/@me/channels";
const REACTIONS: &str = "/channels/*/messages/*/reactions/**";
const QUIET_DM_ID: u64 = 9_000_000_001;
const VERY_HIGH: i64 = 4;

fn strangers(fixtures: &Fixtures) -> Vec<Snowflake> {
	let mut known: HashSet<u64> = fixtures
		.relationships
		.iter()
		.map(|relationship| relationship.id.0)
		.collect();
	for channel_id in &fixtures.private_channel_ids {
		known.extend(fixtures.channels[channel_id].recipient_ids.iter().map(|id| id.0));
	}
	fixtures
		.users
		.values()
		.filter(|user| user.id != fixtures.current_user.id && !known.contains(&user.id.0))
		.map(|user| user.id)
		.collect()
}

fn private_channel(fixtures: &Fixtures, index: usize) -> Snowflake {
	Snowflake(fixtures.private_channel_ids[index])
}

fn mark_private_channel(mock: &Control, index: usize, field: &'static str) {
	mock.transform_ready(move |_, ready| {
		ready["private_channels"][index][field] = json!(true);
	});
}

fn send(running: &Running, channel_id: Snowflake, content: &str) {
	running.session().send(Command::SendMessage {
		channel_id,
		content: content.to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
}

fn confirm(running: &Running, target: RiskyTarget, reason: RiskyReason) {
	running.session().send(Command::ConfirmRisky { target, reason });
}

fn lock_reason(state: &State, channel_id: Snowflake) -> Option<RiskyReason> {
	state.channel_lock(channel_id, Utc::now()).map(|lock| lock.reason)
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

async fn refused(running: &Running, before: u64) {
	running
		.until("the action to be held", |state| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.id > before)
		})
		.await;
}

async fn send_is_held(running: &Running, channel_id: Snowflake, reason: RiskyReason) {
	running
		.until("the lock to show in state", |state| {
			lock_reason(state, channel_id) == Some(reason)
		})
		.await;
	let before = rejections(running);
	send(running, channel_id, "held back");
	refused(running, before).await;
	let path = format!("/channels/{channel_id}/messages");
	assert_eq!(running.mock().recording().requests_to("POST", &path).count(), 0);
	assert_eq!(
		lock_reason(&running.session().state().read(), channel_id),
		Some(reason)
	);
}

async fn send_reaches_wire(running: &Running, channel_id: Snowflake, content: &'static str) {
	send(running, channel_id, content);
	let path = format!("/channels/{channel_id}/messages");
	running
		.until_wire("the send to reach mockcord", |recording| {
			recording
				.requests_to("POST", &path)
				.find(|request| request.json().is_some_and(|body| body["content"] == content))
				.map(drop)
		})
		.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dm_to_a_stranger_is_held_until_the_user_confirms() {
	let running = Running::start("locks-stranger").await;
	running.connected().await;
	let stranger = strangers(running.fixtures())[0];
	running
		.until("the stranger lock", |state| {
			state
				.dm_lock(stranger)
				.is_some_and(|lock| lock.reason == RiskyReason::StrangerDm)
		})
		.await;
	let before = rejections(&running);
	running.session().send(Command::OpenDm { user_id: stranger });
	refused(&running, before).await;
	assert_eq!(running.mock().recording().requests_to("POST", OPEN_DM).count(), 0);

	confirm(&running, RiskyTarget::User(stranger), RiskyReason::StrangerDm);
	running
		.until("the confirmation", |state| state.dm_lock(stranger).is_none())
		.await;
	running.session().send(Command::OpenDm { user_id: stranger });
	running
		.until_wire("the DM to open", |recording| {
			recording.requests_to("POST", OPEN_DM).next().map(drop)
		})
		.await;
	running
		.until("the new DM to be selected", |state| {
			state
				.selected()
				.and_then(|selected| state.channel(selected.channel_id))
				.is_some_and(|channel| {
					channel.kind == ChannelType::Dm && channel.recipient_ids.contains(&stranger)
				})
		})
		.await;
	let dm = running
		.session()
		.state()
		.read()
		.selected()
		.expect("a selected DM")
		.channel_id;
	assert_eq!(lock_reason(&running.session().state().read(), dm), None);
	send_reaches_wire(&running, dm, "hello stranger").await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_first_message_in_an_existing_dm_with_a_stranger_is_held() {
	let running = Running::start("locks-stranger-dm").await;
	running.connected().await;
	let stranger = strangers(running.fixtures())[1];
	running.mock().dispatch(
		"CHANNEL_CREATE",
		&json!({
			"id": QUIET_DM_ID.to_string(),
			"type": 1,
			"recipient_ids": [stranger.to_string()],
			"last_message_id": null,
		}),
	);
	send_is_held(&running, Snowflake(QUIET_DM_ID), RiskyReason::StrangerDm).await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replying_to_a_message_request_is_held() {
	let running = Running::start_with(
		Options::new("locks-request").prepare(|mock| mark_private_channel(mock, 0, "is_message_request")),
	)
	.await;
	running.connected().await;
	let channel_id = private_channel(running.fixtures(), 0);
	send_is_held(&running, channel_id, RiskyReason::MessageRequest).await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replying_in_a_spam_dm_is_held() {
	let running = Running::start_with(
		Options::new("locks-spam").prepare(|mock| mark_private_channel(mock, 1, "is_spam")),
	)
	.await;
	running.connected().await;
	let channel_id = private_channel(running.fixtures(), 1);
	send_is_held(&running, channel_id, RiskyReason::SpamDm).await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn posting_first_in_an_empty_channel_is_held() {
	let running = Running::start("locks-empty").await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running.session().send(Command::CreateChannel {
		guild_id,
		name: "quiet-corner".to_owned(),
		kind: ChannelType::GuildText,
		parent_id: None,
	});
	running
		.until("the new channel", |state| {
			state.guild(guild_id).is_some_and(|guild| {
				guild
					.channels
					.values()
					.any(|channel| channel.name.as_deref() == Some("quiet-corner"))
			})
		})
		.await;
	let channel_id = running
		.session()
		.state()
		.read()
		.guild(guild_id)
		.and_then(|guild| {
			guild
				.channels
				.values()
				.find(|channel| channel.name.as_deref() == Some("quiet-corner"))
				.map(|channel| channel.id)
		})
		.expect("the new channel");
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the empty history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.reached_start && cache.ordered.is_empty())
		})
		.await;
	send_is_held(&running, channel_id, RiskyReason::EmptyChannel).await;
	running.stop().await;
}

fn member_update(fixtures: &Fixtures, guild_id: u64, pending: bool, roles: Value) -> Value {
	json!({
		"guild_id": guild_id.to_string(),
		"user": fixtures.current_user,
		"roles": roles,
		"joined_at": "2025-01-01T00:00:00+00:00",
		"pending": pending,
		"flags": 0,
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_gate_holds_sends_and_reactions() {
	let running = Running::start_with(Options::new("locks-gate").prepare(|mock| {
		mock.transform_ready(|_, ready| {
			ready["user"]["phone"] = Value::Null;
			ready["guilds"][0]["properties"]["verification_level"] = json!(VERY_HIGH);
		});
	}))
	.await;
	running.connected().await;
	let guild = &running.fixtures().guilds[0];
	let guild_id = guild.id;
	let channel_id = Snowflake(guild.message_channel_ids[0]);
	let message_id = *running.fixtures().messages[&channel_id.0]
		.keys()
		.next_back()
		.expect("a fixture message");
	assert_eq!(lock_reason(&running.session().state().read(), channel_id), None);

	let roles = json!(guild.current_member().roles);
	running.mock().dispatch(
		"GUILD_MEMBER_UPDATE",
		&member_update(running.fixtures(), guild_id, true, roles),
	);
	send_is_held(&running, channel_id, RiskyReason::Screening).await;
	let before = rejections(&running);
	running.session().send(Command::AddReaction {
		channel_id,
		message_id: Snowflake(message_id),
		emoji: "👍".to_owned(),
		burst: false,
	});
	refused(&running, before).await;
	assert_eq!(
		running.mock().recording().requests_to("PUT", REACTIONS).count(),
		0
	);

	running.mock().dispatch(
		"GUILD_MEMBER_UPDATE",
		&member_update(running.fixtures(), guild_id, false, json!([])),
	);
	send_is_held(&running, channel_id, RiskyReason::Verification).await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirming_unlocks_exactly_that_channel() {
	let running = Running::start_with(Options::new("locks-confirm").prepare(|mock| {
		mock.transform_ready(|_, ready| {
			ready["private_channels"][0]["is_message_request"] = json!(true);
			ready["private_channels"][1]["is_message_request"] = json!(true);
		});
	}))
	.await;
	running.connected().await;
	let confirmed = private_channel(running.fixtures(), 0);
	let other = private_channel(running.fixtures(), 1);
	running
		.until("both locks", |state| {
			lock_reason(state, confirmed) == Some(RiskyReason::MessageRequest)
				&& lock_reason(state, other) == Some(RiskyReason::MessageRequest)
		})
		.await;
	confirm(
		&running,
		RiskyTarget::Channel(confirmed),
		RiskyReason::MessageRequest,
	);
	running
		.until("the confirmed channel to unlock", |state| {
			lock_reason(state, confirmed).is_none()
		})
		.await;
	send_reaches_wire(&running, confirmed, "confirmed reply").await;
	send_is_held(&running, other, RiskyReason::MessageRequest).await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turning_the_safety_setting_off_lifts_every_lock_after_a_restart() {
	let mut running = Running::start_with(Options::new("locks-setting").prepare(|mock| {
		mock.transform_ready(|_, ready| {
			ready["private_channels"][0]["is_message_request"] = json!(true);
			ready["private_channels"][1]["is_spam"] = json!(true);
		});
	}))
	.await;
	running.connected().await;
	let request = private_channel(running.fixtures(), 0);
	let spam = private_channel(running.fixtures(), 1);
	let stranger = strangers(running.fixtures())[0];
	running
		.until("the locks", |state| {
			state.safety_locks()
				&& lock_reason(state, request).is_some()
				&& lock_reason(state, spam).is_some()
		})
		.await;
	running
		.store()
		.update_settings(|settings| settings.safety_locks = false)
		.expect("settings save");
	assert!(running.session().state().read().safety_locks());

	running.restart().await;
	running.connected().await;
	running
		.until("the locks to be off", |state| {
			!state.safety_locks()
				&& state.dm_lock(stranger).is_none()
				&& lock_reason(state, request).is_none()
				&& lock_reason(state, spam).is_none()
		})
		.await;
	send_reaches_wire(&running, request, "request reply").await;
	send_reaches_wire(&running, spam, "spam reply").await;
	running.session().send(Command::OpenDm { user_id: stranger });
	running
		.until_wire("the stranger DM to open", |recording| {
			recording.requests_to("POST", OPEN_DM).next().map(drop)
		})
		.await;
	running.stop().await;
}
