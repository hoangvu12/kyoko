use std::time::{Duration, Instant};

use mockcord::{Recording, Reply, RestRequest};
use protocol::{ChannelType, Opcode, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::{ReplyTarget, State};

use crate::harness::{Options, Running};

const GAP: Duration = Duration::from_millis(500);
const RETRY_AFTER: f64 = 0.6;
const QUIET: Duration = Duration::from_millis(1500);
const LARGE_GUILD_MEMBERS: usize = 300;
const LIVE_ID_OFFSET: u64 = 1 << 30;
const ROOMY_BUCKET: u32 = 1000;

fn tuning() -> RestTuning {
	RestTuning {
		backfill_gap: GAP,
		..RestTuning::default()
	}
}

fn options(name: &'static str) -> Options {
	Options::new(name)
		.rest(tuning())
		.buckets(ROOMY_BUCKET, Duration::from_secs(1))
}

fn oldest_fixture_messages(running: &Running, channel_id: Snowflake, count: usize) -> Vec<Snowflake> {
	running.fixtures().messages[&channel_id.get()]
		.keys()
		.take(count)
		.map(|id| Snowflake(*id))
		.collect()
}

fn newest_fixture_message(running: &Running, channel_id: Snowflake) -> u64 {
	running.fixtures().messages[&channel_id.get()]
		.keys()
		.next_back()
		.copied()
		.unwrap_or_default()
}

fn live_message(
	running: &Running,
	channel_id: Snowflake,
	guild_id: Option<Snowflake>,
	sequence: u64,
	content: &str,
) -> Value {
	let author = &running.fixtures().current_user;
	let id = newest_fixture_message(running, channel_id) + LIVE_ID_OFFSET + sequence;
	json!({
		"id": id.to_string(),
		"channel_id": channel_id.to_string(),
		"guild_id": guild_id.map(|id| id.to_string()),
		"author": author,
		"content": content,
		"type": 0,
		"timestamp": "2026-01-01T00:00:00Z",
		"mentions": [],
	})
}

fn post_reply(running: &Running, channel_id: Snowflake, sequence: u64, target: Snowflake) -> Snowflake {
	let guild_id = running.session().state().read().channel_guild(channel_id);
	let mut message = live_message(running, channel_id, guild_id, sequence, "a reply");
	message["type"] = json!(19);
	message["message_reference"] = json!({
		"message_id": target.to_string(),
		"channel_id": channel_id.to_string(),
	});
	let id = Snowflake(
		message["id"]
			.as_str()
			.and_then(|id| id.parse().ok())
			.unwrap_or_default(),
	);
	running.mock().dispatch("MESSAGE_CREATE", &message);
	id
}

fn post_mention(running: &Running, channel_id: Snowflake, sequence: u64, users: &[u64]) {
	let guild_id = running.session().state().read().channel_guild(channel_id);
	let content = users
		.iter()
		.map(|id| format!("hi <@{id}>"))
		.collect::<Vec<_>>()
		.join(" ");
	let message = live_message(running, channel_id, guild_id, sequence, &content);
	running.mock().dispatch("MESSAGE_CREATE", &message);
}

fn target_of(state: &State, channel_id: Snowflake, reply_id: Snowflake) -> Option<Option<Snowflake>> {
	let message = state.messages(channel_id)?.by_id.get(&reply_id)?;
	match state.reply_target(message)? {
		ReplyTarget::Message(found) => Some(Some(found.id)),
		ReplyTarget::Unavailable | ReplyTarget::Deleted(_) => Some(None),
		ReplyTarget::Loading => None,
	}
}

fn around_requests(recording: &Recording, channel_id: Snowflake, target: Snowflake) -> Vec<RestRequest> {
	let query = format!("around={target}&limit=1");
	recording
		.requests_to("GET", &format!("/channels/{channel_id}/messages"))
		.filter(|request| request.query.as_deref() == Some(query.as_str()))
		.cloned()
		.collect()
}

fn backfill_requests(recording: &Recording) -> Vec<RestRequest> {
	let mut requests: Vec<RestRequest> = recording
		.requests
		.iter()
		.filter(|request| {
			let around = request.is("GET", "/channels/*/messages")
				&& request
					.query
					.as_deref()
					.is_some_and(|query| query.starts_with("around=") && query.ends_with("&limit=1"));
			let user = request.is("GET", "/users/*") && !request.path.contains('@');
			around || user
		})
		.cloned()
		.collect();
	requests.sort_by_key(|request| request.at);
	requests
}

fn assert_paced(requests: &[RestRequest], triggered: Instant) {
	for (index, request) in requests.iter().enumerate() {
		let after = request.at.saturating_duration_since(triggered);
		assert!(
			after >= GAP * index as u32,
			"backfill fetch {index} is paced, arrived {after:?} after the trigger"
		);
	}
}

fn user_requests(recording: &Recording, user_id: u64) -> Vec<RestRequest> {
	recording
		.requests_to("GET", &format!("/users/{user_id}"))
		.cloned()
		.collect()
}

fn requested_members(recording: &Recording) -> Vec<u64> {
	recording
		.frames_with(Opcode::RequestGuildMembers)
		.flat_map(|frame| frame.data["user_ids"].as_array().cloned().unwrap_or_default())
		.filter_map(|id| id.as_str().and_then(|id| id.parse().ok()))
		.collect()
}

fn unknown_users(running: &Running, state: &State, among: impl Iterator<Item = u64>) -> Vec<u64> {
	let me = running.fixtures().current_user.id.get();
	among
		.filter(|id| *id != me && state.user(Snowflake(*id)).is_none())
		.collect()
}

async fn open_dm(running: &Running) -> Snowflake {
	running.connected().await;
	let channel_id = running
		.session()
		.state()
		.read()
		.private_channels()
		.next()
		.map(|channel| channel.id)
		.expect("a private channel in the fixtures");
	running.session().send(Command::SelectChannel {
		guild_id: None,
		channel_id,
	});
	running
		.until("the DM history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
	channel_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replies_to_unloaded_messages_fetch_each_original_once_and_paced() {
	let running = Running::start_with(options("backfill-replies")).await;
	let channel_id = running.open_text_channel().await;
	let targets = oldest_fixture_messages(&running, channel_id, 2);
	let triggered = Instant::now();
	let replies = [
		post_reply(&running, channel_id, 1, targets[0]),
		post_reply(&running, channel_id, 2, targets[0]),
		post_reply(&running, channel_id, 3, targets[1]),
		post_reply(&running, channel_id, 4, targets[0]),
	];
	running
		.until("every reply to resolve its original", |state| {
			replies
				.iter()
				.zip([0, 0, 1, 0])
				.all(|(reply, target)| target_of(state, channel_id, *reply) == Some(Some(targets[target])))
		})
		.await;
	let recording = running.mock().recording();
	let first = around_requests(&recording, channel_id, targets[0]);
	let second = around_requests(&recording, channel_id, targets[1]);
	assert_eq!(first.len(), 1, "one fetch for the shared original");
	assert_eq!(second.len(), 1, "one fetch for the other original");
	assert_paced(&backfill_requests(&recording), triggered);
	let resident = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.is_some_and(|cache| targets.iter().any(|target| cache.by_id.contains_key(target)));
	assert!(!resident, "fetched originals stay out of the channel window");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_original_is_marked_unavailable_and_not_fetched_again() {
	let running = Running::start_with(options("backfill-missing")).await;
	let channel_id = running.open_text_channel().await;
	let target = oldest_fixture_messages(&running, channel_id, 1)[0];
	running.mock().respond_once(
		"GET",
		&format!("/channels/{channel_id}/messages"),
		Reply::error(404, 10008, "Unknown Message"),
	);
	let first = post_reply(&running, channel_id, 1, target);
	running
		.until("the reply to show its original as unavailable", |state| {
			target_of(state, channel_id, first) == Some(None)
		})
		.await;
	let second = post_reply(&running, channel_id, 2, target);
	running
		.until("the second reply to arrive", |state| {
			target_of(state, channel_id, second) == Some(None)
		})
		.await;
	tokio::time::sleep(QUIET).await;
	let fetches = around_requests(&running.mock().recording(), channel_id, target);
	assert_eq!(fetches.len(), 1, "an unavailable original is never fetched again");
	assert_eq!(fetches[0].status, 404);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rate_limited_backfill_waits_before_trying_again() {
	let running = Running::start_with(options("backfill-rate-limited")).await;
	let channel_id = running.open_text_channel().await;
	let target = oldest_fixture_messages(&running, channel_id, 1)[0];
	running.mock().respond_times(
		"GET",
		&format!("/channels/{channel_id}/messages"),
		Reply::rate_limited(RETRY_AFTER, false),
		2,
	);
	let reply = post_reply(&running, channel_id, 1, target);
	running
		.until("the reply to resolve after the rate limit", |state| {
			target_of(state, channel_id, reply) == Some(Some(target))
		})
		.await;
	let fetches = around_requests(&running.mock().recording(), channel_id, target);
	assert_eq!(
		fetches.len(),
		3,
		"the client retry, then one backfill retry: {fetches:?}"
	);
	assert_eq!(fetches[1].status, 429);
	let wait = fetches[2].at.saturating_duration_since(fetches[1].at);
	assert!(
		wait >= Duration::from_secs_f64(RETRY_AFTER),
		"the queue waits for retry_after, got {wait:?}"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_backfill_that_stays_rate_limited_gives_up_without_a_retry_loop() {
	let running = Running::start_with(options("backfill-rate-limit-loop")).await;
	let channel_id = running.open_text_channel().await;
	let target = oldest_fixture_messages(&running, channel_id, 1)[0];
	running.mock().respond_always(
		"GET",
		&format!("/channels/{channel_id}/messages"),
		Reply::rate_limited(RETRY_AFTER, false),
	);
	let reply = post_reply(&running, channel_id, 1, target);
	running
		.until_wire("the backfill retry", |recording| {
			(around_requests(recording, channel_id, target).len() >= 4).then_some(())
		})
		.await;
	tokio::time::sleep(QUIET).await;
	let fetches = around_requests(&running.mock().recording(), channel_id, target);
	assert_eq!(
		fetches.len(),
		4,
		"two attempts of a client retry each, then nothing"
	);
	let wait = fetches[2].at.saturating_duration_since(fetches[1].at);
	assert!(
		wait >= Duration::from_secs_f64(RETRY_AFTER),
		"the queue waits for retry_after, got {wait:?}"
	);
	let pending = target_of(&running.session().state().read(), channel_id, reply).is_none();
	assert!(pending, "a rate limit does not mark the original unavailable");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaving_a_channel_drops_its_queued_backfill() {
	let slow = RestTuning {
		backfill_gap: Duration::from_secs(1),
		..RestTuning::default()
	};
	let running = Running::start_with(options("backfill-leave").rest(slow)).await;
	let channel_id = running.open_text_channel().await;
	let targets = oldest_fixture_messages(&running, channel_id, 3);
	let replies: Vec<Snowflake> = targets
		.iter()
		.enumerate()
		.map(|(index, target)| post_reply(&running, channel_id, index as u64 + 1, *target))
		.collect();
	running
		.until("the replies to arrive", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| replies.iter().all(|reply| cache.by_id.contains_key(reply)))
		})
		.await;
	let (guild_id, other) = {
		let state = running.session().state().read();
		let guild_id = state.channel_guild(channel_id).expect("a guild channel");
		let other = state
			.guild(guild_id)
			.and_then(|guild| {
				guild
					.channels
					.values()
					.find(|channel| channel.kind == ChannelType::GuildText && channel.id != channel_id)
					.map(|channel| channel.id)
			})
			.expect("a second text channel");
		(guild_id, other)
	};
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id: other,
	});
	running
		.until("the other channel to load", |state| {
			state
				.messages(other)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
	tokio::time::sleep(QUIET * 2).await;
	let recording = running.mock().recording();
	let fetched: usize = targets
		.iter()
		.map(|target| around_requests(&recording, channel_id, *target).len())
		.sum();
	assert!(
		fetched <= 1,
		"only the fetch already in flight is sent, got {fetched}"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_mention_in_a_guild_is_requested_over_the_gateway() {
	let running = Running::start_with(options("backfill-guild-mention").members(LARGE_GUILD_MEMBERS)).await;
	let channel_id = running.open_text_channel().await;
	let guild_id = running
		.session()
		.state()
		.read()
		.channel_guild(channel_id)
		.expect("a guild channel");
	let user_id = {
		let state = running.session().state().read();
		let fixture = running.fixtures().guild(guild_id.get()).expect("a fixture guild");
		unknown_users(
			&running,
			&state,
			fixture
				.members
				.iter()
				.filter_map(|member| member.user.as_ref().map(|user| user.id.get())),
		)
		.pop()
		.expect("a member kyoko has not seen")
	};
	post_mention(&running, channel_id, 1, &[user_id]);
	running
		.until_wire("an op 8 request for the mentioned user", |recording| {
			requested_members(recording).contains(&user_id).then_some(())
		})
		.await;
	running
		.until("the mentioned user to resolve", |state| {
			state.user(Snowflake(user_id)).is_some()
		})
		.await;
	assert!(
		user_requests(&running.mock().recording(), user_id).is_empty(),
		"guild users are looked up over the gateway, not REST"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_mentions_in_a_dm_are_fetched_over_rest_and_paced() {
	let running = Running::start_with(options("backfill-dm-mention")).await;
	let channel_id = open_dm(&running).await;
	let users = {
		let state = running.session().state().read();
		let mut unknown = unknown_users(&running, &state, running.fixtures().users.keys().copied());
		unknown.truncate(2);
		unknown
	};
	assert_eq!(users.len(), 2, "two users kyoko has not seen");
	let triggered = Instant::now();
	post_mention(&running, channel_id, 1, &users);
	running
		.until("both mentioned users to resolve", |state| {
			users.iter().all(|id| state.user(Snowflake(*id)).is_some())
		})
		.await;
	let recording = running.mock().recording();
	let first = user_requests(&recording, users[0]);
	let second = user_requests(&recording, users[1]);
	assert_eq!((first.len(), second.len()), (1, 1), "one fetch per user");
	assert_paced(&backfill_requests(&recording), triggered);
	assert!(
		!requested_members(&recording).iter().any(|id| users.contains(id)),
		"DM users are not requested over op 8"
	);
	running.stop().await;
}
