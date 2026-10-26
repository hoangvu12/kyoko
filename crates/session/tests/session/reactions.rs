use std::time::Duration;

use mockcord::{BURST_COLORS, Recording};
use protocol::{ChannelType, Reaction, Snowflake};
use serde_json::json;
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const GUILDS: usize = 3;
const MEMBER_GUILD: usize = 2;
const TEST_TUBE: &str = "\u{1f9ea}";
const SPARKLES: &str = "\u{2728}";
const PAGE: usize = 100;

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

fn reaction(state: &State, channel_id: Snowflake, message_id: Snowflake, emoji: &str) -> Option<Reaction> {
	state
		.messages(channel_id)?
		.by_id
		.get(&message_id)?
		.reactions
		.iter()
		.find(|reaction| reaction.emoji.name.as_deref() == Some(emoji))
		.cloned()
}

fn requests<'a>(recording: &'a Recording, method: &'a str, pattern: &'a str) -> Vec<Option<String>> {
	recording
		.requests_to(method, pattern)
		.map(|request| request.query.clone())
		.collect()
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

fn rejected_with(state: &State, before: u64, words: &str) -> bool {
	state
		.last_rejection()
		.is_some_and(|rejection| rejection.id > before && rejection.reason.contains(words))
}

fn others(running: &Running) -> Vec<u64> {
	let me = running.fixtures().current_user.id.get();
	running
		.fixtures()
		.users
		.keys()
		.copied()
		.filter(|id| *id != me)
		.collect()
}

async fn open(running: &Running) -> (Snowflake, Snowflake) {
	let channel_id = running.open_text_channel().await;
	let message_id = newest(&running.session().state().read(), channel_id).expect("a message");
	(channel_id, message_id)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_super_reaction_is_added_and_removed_with_the_burst_type() {
	let running = Running::start_with(Options::new("reactions-burst").rest(tuning())).await;
	let (channel_id, message_id) = open(&running).await;
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: SPARKLES.to_owned(),
		burst: true,
	});
	running
		.until("the super reaction to echo back", |state| {
			reaction(state, channel_id, message_id, SPARKLES)
				.is_some_and(|reaction| reaction.me_burst && reaction.burst_count() == 1 && !reaction.me)
		})
		.await;
	let added = requests(
		&running.mock().recording(),
		"PUT",
		"/channels/*/messages/*/reactions/*/@me",
	);
	assert_eq!(
		added,
		[Some("type=1".to_owned())],
		"a super reaction is sent as type 1"
	);
	{
		let state = running.session().state().read();
		let shown = reaction(&state, channel_id, message_id, SPARKLES).expect("the reaction");
		assert_eq!(shown.normal_count(), 0);
		let colors: Vec<&str> = shown.burst_colors.iter().map(String::as_str).collect();
		assert_eq!(colors, BURST_COLORS, "the burst colours arrive with the event");
	}
	running.session().send(Command::RemoveReaction {
		channel_id,
		message_id,
		emoji: SPARKLES.to_owned(),
		burst: true,
	});
	running
		.until("the super reaction to go", |state| {
			reaction(state, channel_id, message_id, SPARKLES).is_none()
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(
		recording
			.requests_to("DELETE", "/channels/*/messages/*/reactions/*/1/@me")
			.count(),
		1,
		"the super reaction is removed on the burst route"
	);
	assert_eq!(
		recording
			.requests_to("DELETE", "/channels/*/messages/*/reactions/*/0/@me")
			.count(),
		0
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn gateway_burst_events_count_apart_from_normal_reactions() {
	let running = Running::start_with(Options::new("reactions-gateway-burst").rest(tuning())).await;
	let (channel_id, message_id) = open(&running).await;
	let guild_id = running
		.session()
		.state()
		.read()
		.channel_guild(channel_id)
		.expect("a guild channel");
	let someone = others(&running)[0];
	let event = |burst: serde_json::Value, kind: serde_json::Value| {
		json!({
			"user_id": someone.to_string(),
			"channel_id": channel_id.to_string(),
			"message_id": message_id.to_string(),
			"guild_id": guild_id.to_string(),
			"emoji": { "id": null, "name": TEST_TUBE },
			"burst": burst,
			"type": kind,
			"burst_colors": ["#ff0000", "not a colour", 7, "#0000ff"],
		})
	};
	running
		.mock()
		.dispatch("MESSAGE_REACTION_ADD", &event(json!(true), json!(1)));
	running
		.mock()
		.dispatch("MESSAGE_REACTION_ADD", &event(json!(null), json!(0)));
	running
		.until("a super and a normal reaction", |state| {
			reaction(state, channel_id, message_id, TEST_TUBE)
				.is_some_and(|reaction| reaction.burst_count() == 1 && reaction.normal_count() == 1)
		})
		.await;
	{
		let state = running.session().state().read();
		let shown = reaction(&state, channel_id, message_id, TEST_TUBE).expect("the reaction");
		assert_eq!(shown.count, 2);
		assert!(!shown.me && !shown.me_burst, "someone else reacted");
		let colors: Vec<&str> = shown.burst_colors.iter().map(String::as_str).collect();
		assert_eq!(
			colors,
			["#ff0000", "not a colour", "#0000ff"],
			"a bad element is dropped"
		);
	}
	running
		.mock()
		.dispatch("MESSAGE_REACTION_REMOVE", &event(json!(true), json!(null)));
	running
		.until("the super reaction to go and the normal one to stay", |state| {
			reaction(state, channel_id, message_id, TEST_TUBE)
				.is_some_and(|reaction| reaction.burst_count() == 0 && reaction.normal_count() == 1)
		})
		.await;
	running
		.mock()
		.dispatch("MESSAGE_REACTION_REMOVE", &event(json!(false), json!(0)));
	running
		.until("the last reaction to go", |state| {
			reaction(state, channel_id, message_id, TEST_TUBE).is_none()
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_nitro_a_super_reaction_never_reaches_rest() {
	let options = Options::new("reactions-no-nitro")
		.rest(tuning())
		.prepare(|mock| mock.transform_ready(|_, ready| ready["user"]["premium_type"] = json!(0)));
	let running = Running::start_with(options).await;
	let (channel_id, message_id) = open(&running).await;
	let before = rejections(&running);
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: SPARKLES.to_owned(),
		burst: true,
	});
	running
		.until("the super reaction to be refused", |state| {
			rejected_with(state, before, "Nitro")
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(
		recording
			.requests_to("PUT", "/channels/*/messages/*/reactions/**")
			.count(),
		0
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_reactions_list_loads_page_by_page_for_each_type() {
	let running = Running::start_with(Options::new("reactions-list").rest(tuning())).await;
	let (channel_id, message_id) = open(&running).await;
	let me = running.fixtures().current_user.id.get();
	let reactors: Vec<u64> = others(&running).into_iter().chain([me]).collect();
	assert!(reactors.len() > PAGE, "more reactors than one page");
	for user_id in &reactors {
		assert!(
			running
				.mock()
				.react_as(channel_id.get(), message_id.get(), *user_id, TEST_TUBE, false)
		);
	}
	let fans = &reactors[..3];
	for user_id in fans {
		assert!(
			running
				.mock()
				.react_as(channel_id.get(), message_id.get(), *user_id, TEST_TUBE, true)
		);
	}
	running
		.until("every reaction to arrive", |state| {
			reaction(state, channel_id, message_id, TEST_TUBE).is_some_and(|reaction| {
				reaction.normal_count() == reactors.len() as i64 && reaction.burst_count() == 3
			})
		})
		.await;
	let load = |burst: bool, after: Option<Snowflake>| Command::LoadReactionUsers {
		channel_id,
		message_id,
		emoji: TEST_TUBE.to_owned(),
		burst,
		after,
	};
	running.session().send(load(false, None));
	running
		.until("the first page", |state| {
			state
				.reactors(message_id, TEST_TUBE, false)
				.is_some_and(|list| list.users.len() == PAGE && list.has_more)
		})
		.await;
	let after = running
		.session()
		.state()
		.read()
		.reactors(message_id, TEST_TUBE, false)
		.and_then(|list| list.users.iter().map(|user| user.id).max());
	running.session().send(load(false, after));
	running
		.until("the second page", |state| {
			state
				.reactors(message_id, TEST_TUBE, false)
				.is_some_and(|list| list.users.len() == reactors.len() && !list.has_more)
		})
		.await;
	running.session().send(load(true, None));
	running
		.until("the super reactors", |state| {
			state
				.reactors(message_id, TEST_TUBE, true)
				.is_some_and(|list| list.users.len() == fans.len() && !list.has_more)
		})
		.await;
	let queries = requests(
		&running.mock().recording(),
		"GET",
		"/channels/*/messages/*/reactions/*",
	);
	let after = after.expect("a last reactor");
	assert_eq!(
		queries,
		[
			Some("limit=100&type=0".to_owned()),
			Some(format!("limit=100&after={after}&type=0")),
			Some("limit=100&type=1".to_owned()),
		]
	);
	let mut listed: Vec<u64> = running
		.session()
		.state()
		.read()
		.reactors(message_id, TEST_TUBE, false)
		.expect("the list")
		.users
		.iter()
		.map(|user| user.id.get())
		.collect();
	listed.sort_unstable();
	let mut expected = reactors.clone();
	expected.sort_unstable();
	assert_eq!(listed, expected, "no one is listed twice or left out");
	running.stop().await;
}

async fn member_channel(running: &Running) -> Snowflake {
	running.connected().await;
	let guild = &running.fixtures().guilds[MEMBER_GUILD];
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
		guild_id: Some(Snowflake(guild.id)),
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
async fn removing_someone_elses_reaction_needs_manage_messages() {
	let running = Running::start_with(Options::new("reactions-moderate").guilds(GUILDS).rest(tuning())).await;
	let someone = others(&running)[0];
	let member_channel = member_channel(&running).await;
	let member_message = newest(&running.session().state().read(), member_channel).expect("a message");
	assert!(
		!running
			.session()
			.state()
			.read()
			.can_manage_messages(member_channel)
	);
	let before = rejections(&running);
	running.session().send(Command::RemoveUserReaction {
		channel_id: member_channel,
		message_id: member_message,
		emoji: TEST_TUBE.to_owned(),
		burst: false,
		user_id: Snowflake(someone),
	});
	running
		.until("the removal to be refused", |state| {
			rejected_with(state, before, "Manage Messages")
		})
		.await;
	assert_eq!(
		running
			.mock()
			.recording()
			.requests_to("DELETE", "/channels/*/messages/*/reactions/**")
			.count(),
		0,
		"no request without Manage Messages"
	);
	let (channel_id, message_id) = open(&running).await;
	assert!(running.session().state().read().can_manage_messages(channel_id));
	for burst in [false, true] {
		assert!(
			running
				.mock()
				.react_as(channel_id.get(), message_id.get(), someone, TEST_TUBE, burst)
		);
	}
	running
		.until("their reactions to arrive", |state| {
			reaction(state, channel_id, message_id, TEST_TUBE)
				.is_some_and(|reaction| reaction.normal_count() == 1 && reaction.burst_count() == 1)
		})
		.await;
	running.session().send(Command::LoadReactionUsers {
		channel_id,
		message_id,
		emoji: TEST_TUBE.to_owned(),
		burst: true,
		after: None,
	});
	running
		.until("the super reactors", |state| {
			state
				.reactors(message_id, TEST_TUBE, true)
				.is_some_and(|list| list.users.len() == 1)
		})
		.await;
	running.session().send(Command::RemoveUserReaction {
		channel_id,
		message_id,
		emoji: TEST_TUBE.to_owned(),
		burst: true,
		user_id: Snowflake(someone),
	});
	running
		.until(
			"their super reaction to go, from the message and the list",
			|state| {
				reaction(state, channel_id, message_id, TEST_TUBE)
					.is_some_and(|reaction| reaction.normal_count() == 1 && reaction.burst_count() == 0)
					&& state
						.reactors(message_id, TEST_TUBE, true)
						.is_some_and(|list| list.users.is_empty())
			},
		)
		.await;
	let route = format!("/channels/{channel_id}/messages/{message_id}/reactions/*/1/{someone}");
	assert_eq!(
		running.mock().recording().requests_to("DELETE", &route).count(),
		1,
		"one removal on the burst route for that user"
	);
	running.stop().await;
}
