use std::time::Duration;

use chrono::Utc;
use mockcord::{Recording, Reply};
use protocol::{ChannelType, Snowflake};
use serde_json::json;
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const GUILDS: usize = 3;
const MEMBER_GUILD: usize = 2;
const THUMBS_UP: &str = "\u{1f44d}";
const PARTY: &str = "\u{1f389}";

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn newest(state: &State, channel_id: Snowflake) -> Option<Snowflake> {
	state
		.messages(channel_id)
		.and_then(|cache| cache.ordered.last().map(|message| message.id))
}

fn pinned(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Option<bool> {
	state
		.messages(channel_id)
		.and_then(|cache| cache.by_id.get(&message_id))
		.map(|message| message.pinned)
}

fn rejected_with(state: &State, before: u64, words: &str) -> bool {
	state
		.last_rejection()
		.is_some_and(|rejection| rejection.id > before && rejection.reason.contains(words))
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

fn count(recording: &Recording, method: &str, pattern: &str) -> usize {
	recording.requests_to(method, pattern).count()
}

async fn open_member_channel(running: &Running) -> Snowflake {
	running.connected().await;
	let guild = &running.fixtures().guilds[MEMBER_GUILD];
	let guild_id = Snowflake(guild.id);
	let channel_id = guild
		.message_channel_ids
		.iter()
		.map(|id| Snowflake(*id))
		.find(|id| {
			running
				.fixtures()
				.channels
				.get(&id.get())
				.is_some_and(|channel| channel.kind == ChannelType::GuildText)
		})
		.expect("a text channel in the member guild");
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the member channel history to load", |state| {
			newest(state, channel_id).is_some()
		})
		.await;
	channel_id
}

#[tokio::test(flavor = "multi_thread")]
async fn pinning_and_unpinning_reach_the_channel_and_echo_back() {
	let running = Running::start_with(Options::new("actions-pin").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let message_id = newest(&running.session().state().read(), channel_id).expect("a message");
	running.session().send(Command::PinMessage {
		channel_id,
		message_id,
	});
	running
		.until("the pin to echo back", |state| {
			pinned(state, channel_id, message_id) == Some(true)
		})
		.await;
	running.session().send(Command::UnpinMessage {
		channel_id,
		message_id,
	});
	running
		.until("the unpin to echo back", |state| {
			pinned(state, channel_id, message_id) == Some(false)
		})
		.await;
	let recording = running.mock().recording();
	let route = format!("/channels/{channel_id}/messages/pins/{message_id}");
	assert_eq!(count(&recording, "PUT", &route), 1, "one pin request");
	assert_eq!(count(&recording, "DELETE", &route), 1, "one unpin request");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pins_and_new_reactions_without_permission_never_reach_rest() {
	let running = Running::start_with(Options::new("actions-refused").guilds(GUILDS).rest(tuning())).await;
	let channel_id = open_member_channel(&running).await;
	let message_id = {
		let state = running.session().state().read();
		assert!(
			!state.can_manage_messages(channel_id),
			"the member cannot manage messages"
		);
		assert!(
			!state.can_react(channel_id),
			"the muted member cannot add reactions"
		);
		assert!(
			state.channel_lock(channel_id, Utc::now()).is_none(),
			"no safety lock holds the channel"
		);
		newest(&state, channel_id).expect("a message")
	};
	let before = rejections(&running);
	running.session().send(Command::PinMessage {
		channel_id,
		message_id,
	});
	running
		.until("the pin to be refused", |state| {
			rejected_with(state, before, "Manage Messages")
		})
		.await;
	let before = rejections(&running);
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: PARTY.to_owned(),
	});
	running
		.until("the new reaction to be refused", |state| {
			rejected_with(state, before, "Add Reactions")
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(count(&recording, "PUT", "/channels/*/messages/pins/*"), 0);
	assert_eq!(
		count(&recording, "PUT", "/channels/*/messages/*/reactions/*/@me"),
		0
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn joining_an_existing_reaction_needs_only_history() {
	let running = Running::start_with(Options::new("actions-join").guilds(GUILDS).rest(tuning())).await;
	let channel_id = open_member_channel(&running).await;
	let (guild_id, message_id) = {
		let state = running.session().state().read();
		(
			state.channel_guild(channel_id).expect("a guild channel"),
			newest(&state, channel_id).expect("a message"),
		)
	};
	let someone = running
		.fixtures()
		.users
		.keys()
		.copied()
		.find(|id| *id != running.fixtures().current_user.id.get())
		.expect("another user");
	running.mock().dispatch(
		"MESSAGE_REACTION_ADD",
		&json!({
			"user_id": someone.to_string(),
			"channel_id": channel_id.to_string(),
			"message_id": message_id.to_string(),
			"guild_id": guild_id.to_string(),
			"emoji": { "id": null, "name": THUMBS_UP },
		}),
	);
	running
		.until("the other user's reaction to arrive", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&message_id))
				.is_some_and(|message| {
					message
						.reactions
						.iter()
						.any(|reaction| reaction.emoji.name.as_deref() == Some(THUMBS_UP))
				})
		})
		.await;
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: THUMBS_UP.to_owned(),
	});
	running
		.until_wire("the joined reaction request", |recording| {
			(count(recording, "PUT", "/channels/*/messages/*/reactions/*/@me") > 0).then_some(())
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_reaction_reports_why() {
	let running = Running::start_with(Options::new("actions-reaction-failed").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let message_id = newest(&running.session().state().read(), channel_id).expect("a message");
	running.mock().respond_once(
		"PUT",
		"/channels/*/messages/*/reactions/*/@me",
		Reply::missing_permissions(),
	);
	let before = rejections(&running);
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: PARTY.to_owned(),
	});
	running
		.until("the refusal to be reported", |state| {
			rejected_with(state, before, "add reaction")
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(
		count(&recording, "PUT", "/channels/*/messages/*/reactions/*/@me"),
		1,
		"a refused reaction is not retried"
	);
	running.stop().await;
}
