use std::time::Duration;

use mockcord::{GatewayFrame, Recording};
use protocol::{ChannelType, Opcode, Snowflake};
use serde_json::{Value, json};
use session::Command;
use state::{MemberSlot, State};

use crate::harness::{Options, Running};

const LIST_ID: &str = "everyone";
const LARGE_GUILD_MEMBERS: usize = 600;
const RETRY_AFTER: f64 = 0.3;
const FAKE_USER_BASE: u64 = 990_000_000_000_100_000;

fn text_channels(state: &State, guild_id: Snowflake) -> Vec<Snowflake> {
	state
		.guild(guild_id)
		.map(|guild| {
			guild
				.channels
				.values()
				.filter(|channel| channel.kind == ChannelType::GuildText)
				.map(|channel| channel.id)
				.collect()
		})
		.unwrap_or_default()
}

fn guild_ids(running: &Running) -> Vec<Snowflake> {
	running
		.fixtures()
		.guilds
		.iter()
		.map(|guild| Snowflake(guild.id))
		.collect()
}

fn subscription_frames(recording: &Recording) -> Vec<GatewayFrame> {
	recording
		.frames_with(Opcode::GuildSubscriptionsBulk)
		.cloned()
		.collect()
}

fn member_request_frames(recording: &Recording) -> Vec<GatewayFrame> {
	recording
		.frames_with(Opcode::RequestGuildMembers)
		.cloned()
		.collect()
}

fn requested_ids(frame: &GatewayFrame) -> Vec<u64> {
	frame.data["user_ids"]
		.as_array()
		.map(|ids| {
			ids.iter()
				.filter_map(|id| id.as_str().and_then(|id| id.parse().ok()))
				.collect()
		})
		.unwrap_or_default()
}

fn fake_ids(from: u64, count: u64) -> Vec<Snowflake> {
	(from..from + count)
		.map(|offset| Snowflake(FAKE_USER_BASE + offset))
		.collect()
}

async fn open_channel(running: &Running, guild_id: Snowflake, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the member list to sync", |state| {
			state
				.member_list(guild_id)
				.is_some_and(|list| !list.slots.is_empty())
		})
		.await;
}

fn slots(state: &State, guild_id: Snowflake) -> Vec<MemberSlot> {
	state
		.member_list(guild_id)
		.map(|list| list.slots.clone())
		.unwrap_or_default()
}

fn member_item(running: &Running, user_id: u64) -> Value {
	let user = running.fixtures().users.get(&user_id).expect("a fixture user");
	json!({ "member": { "user": user, "roles": [], "presence": { "status": "online", "activities": [] } } })
}

fn list_update(guild_id: Snowflake, list_id: &str, ops: Value) -> Value {
	json!({
		"guild_id": guild_id.to_string(),
		"id": list_id,
		"ops": ops,
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quick_subscription_changes_coalesce_and_unchanged_state_is_not_resent() {
	let running = Running::start("subscriptions-debounce").await;
	running.connected().await;
	let guilds = guild_ids(&running);
	let (guild_id, other_guild) = (guilds[0], guilds[1]);
	let channels = text_channels(&running.session().state().read(), guild_id);
	let (first, second) = (channels[0], channels[1]);
	for channel_id in [first, second] {
		running.session().send(Command::SelectChannel {
			guild_id: Some(guild_id),
			channel_id,
		});
	}
	running.session().send(Command::SetMemberViewport {
		guild_id,
		channel_id: second,
		first: 0,
		last: 20,
	});
	running
		.until_wire("the first guild subscription", |recording| {
			(!subscription_frames(recording).is_empty()).then_some(())
		})
		.await;
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id: second,
	});
	running.session().send(Command::SetMemberViewport {
		guild_id,
		channel_id: second,
		first: 0,
		last: 20,
	});
	running.session().send(Command::SubscribeGuild { guild_id });
	running.session().send(Command::SubscribeGuild {
		guild_id: other_guild,
	});
	let frames = running
		.until_wire("the other guild's subscription", |recording| {
			let frames = subscription_frames(recording);
			frames
				.iter()
				.any(|frame| frame.data["subscriptions"].get(other_guild.to_string()).is_some())
				.then_some(frames)
		})
		.await;
	assert_eq!(frames.len(), 2, "{frames:?}");
	let opened = &frames[0].data["subscriptions"];
	assert_eq!(opened.as_object().map(|guilds| guilds.len()), Some(1));
	let entry = &opened[guild_id.to_string()];
	assert_eq!(entry["typing"], json!(true));
	assert_eq!(entry["activities"], json!(true));
	assert_eq!(entry["threads"], json!(true));
	assert_eq!(
		entry["channels"],
		json!({ second.to_string(): [[0, 99], [100, 199]] })
	);
	let later = &frames[1].data["subscriptions"];
	assert_eq!(later.as_object().map(|guilds| guilds.len()), Some(1));
	assert!(later[other_guild.to_string()]["channels"].is_object());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scrolling_the_member_list_subscribes_the_buckets_around_the_viewport() {
	let running =
		Running::start_with(Options::new("subscriptions-buckets").members(LARGE_GUILD_MEMBERS)).await;
	running.connected().await;
	let guild_id = guild_ids(&running)[0];
	let channel_id = text_channels(&running.session().state().read(), guild_id)[0];
	open_channel(&running, guild_id, channel_id).await;
	running.session().send(Command::SetMemberViewport {
		guild_id,
		channel_id,
		first: 450,
		last: 470,
	});
	let frame = running
		.until_wire("the scrolled member ranges", |recording| {
			subscription_frames(recording).into_iter().find(|frame| {
				frame.data["subscriptions"][guild_id.to_string()]["channels"][channel_id.to_string()]
					.as_array()
					.is_some_and(|ranges| ranges.len() > 1 && ranges[1] == json!([400, 499]))
			})
		})
		.await;
	let entry = &frame.data["subscriptions"][guild_id.to_string()];
	assert_eq!(
		entry,
		&json!({ "channels": { channel_id.to_string(): [[0, 99], [400, 499], [500, 599]] } })
	);
	running
		.until("rows around the viewport to load", |state| {
			let rows = state.member_list_rows(guild_id, 400, 599);
			rows.len() == 200 && rows.iter().all(Option::is_some)
		})
		.await;
	assert!(
		running
			.session()
			.state()
			.read()
			.member_list_rows(guild_id, 0, 99)
			.iter()
			.all(Option::is_some)
	);
	running.session().send(Command::SetMemberViewport {
		guild_id,
		channel_id,
		first: 0,
		last: 30,
	});
	let back = running
		.until_wire("the ranges after scrolling back", |recording| {
			let frames = subscription_frames(recording);
			(frames.len() >= 3).then(|| frames[frames.len() - 1].clone())
		})
		.await;
	assert_eq!(
		back.data["subscriptions"][guild_id.to_string()]["channels"][channel_id.to_string()],
		json!([[0, 99], [100, 199]])
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_member_list_op_is_applied_to_the_published_list() {
	let running = Running::start("member-list-ops").await;
	running.connected().await;
	let guild_id = guild_ids(&running)[0];
	let channel_id = text_channels(&running.session().state().read(), guild_id)[0];
	open_channel(&running, guild_id, channel_id).await;
	let original = slots(&running.session().state().read(), guild_id);
	let (newcomer, replacement) = {
		let mut outsiders = running
			.fixtures()
			.users
			.keys()
			.copied()
			.filter(|id| !original.contains(&MemberSlot::Member(Snowflake(*id))));
		(
			outsiders.next().expect("a user outside the list"),
			outsiders.next().expect("a second user outside the list"),
		)
	};
	let mock = running.mock();

	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&list_update(
			guild_id,
			LIST_ID,
			json!([{ "op": "INSERT", "index": 1, "item": member_item(&running, newcomer) }]),
		),
	);
	running
		.until("the inserted row", |state| {
			let now = slots(state, guild_id);
			now.len() == original.len() + 1
				&& now[1] == MemberSlot::Member(Snowflake(newcomer))
				&& now[2..] == original[1..]
		})
		.await;

	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&list_update(
			guild_id,
			LIST_ID,
			json!([{ "op": "UPDATE", "index": 1, "item": member_item(&running, replacement) }]),
		),
	);
	running
		.until("the updated row", |state| {
			slots(state, guild_id).get(1) == Some(&MemberSlot::Member(Snowflake(replacement)))
		})
		.await;

	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&list_update(guild_id, LIST_ID, json!([{ "op": "DELETE", "index": 1 }])),
	);
	running
		.until("the deleted row", |state| slots(state, guild_id) == original)
		.await;

	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&list_update(guild_id, "another-list", json!([{ "op": "DELETE", "index": 0 }])),
	);
	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&list_update(
			guild_id,
			LIST_ID,
			json!([{ "op": "SURPRISE", "index": 0 }, { "op": "INVALIDATE", "range": [0, 9] }]),
		),
	);
	running
		.until("the invalidated range", |state| {
			let rows = state.member_list_rows(guild_id, 0, 9);
			rows.len() == 10 && rows.iter().all(Option::is_none)
		})
		.await;
	{
		let state = running.session().state().read();
		let now = slots(&state, guild_id);
		assert_eq!(now.len(), original.len());
		assert_eq!(now[10..], original[10..]);
		assert_eq!(
			state.member_list(guild_id).map(|list| list.id.as_str()),
			Some(LIST_ID)
		);
	}

	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&json!({
			"guild_id": guild_id.to_string(),
			"id": LIST_ID,
			"member_count": 2,
			"online_count": 2,
			"groups": [{ "id": "online", "count": 2 }],
			"ops": [{
				"op": "SYNC",
				"range": [0, 99],
				"items": [
					{ "group": { "id": "online", "count": 2 } },
					member_item(&running, newcomer),
					{ "member": "not a member" },
					member_item(&running, replacement),
				],
			}],
		}),
	);
	running
		.until("the resynced list", |state| {
			slots(state, guild_id)
				== [
					MemberSlot::Group {
						id: "online".to_owned(),
						count: 2,
					},
					MemberSlot::Member(Snowflake(newcomer)),
					MemberSlot::Empty,
					MemberSlot::Member(Snowflake(replacement)),
				]
		})
		.await;
	{
		let state = running.session().state().read();
		let list = state.member_list(guild_id).expect("the list");
		assert_eq!(list.group_count("online"), Some(2));
		assert_eq!(list.row_count(), 4);
		assert!(state.member_list_rows(guild_id, 0, 3)[2].is_none());
		assert!(state.user(Snowflake(newcomer)).is_some());
	}

	mock.dispatch(
		"GUILD_MEMBER_LIST_UPDATE",
		&list_update(
			guild_id,
			"another-list",
			json!([{ "op": "SYNC", "range": [0, 99], "items": [member_item(&running, newcomer)] }]),
		),
	);
	running
		.until("a new list to replace the old one", |state| {
			state
				.member_list(guild_id)
				.is_some_and(|list| list.id == "another-list" && list.slots.len() == 1)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn member_requests_are_batched_and_never_repeat_known_ids() {
	let running = Running::start("member-requests-batch").await;
	running.connected().await;
	let guilds = guild_ids(&running);
	let (guild_id, search_guild) = (guilds[0], guilds[1]);
	let members: Vec<Snowflake> = running.fixtures().guilds[0]
		.members
		.iter()
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.collect();
	let fakes = fake_ids(0, 150 - members.len() as u64);
	let mut wanted = members;
	wanted.extend(fakes);
	let (first_half, second_half) = wanted.split_at(70);
	running.session().send(Command::RequestMembers {
		guild_id,
		user_ids: first_half.to_vec(),
	});
	running.session().send(Command::RequestMembers {
		guild_id,
		user_ids: second_half.to_vec(),
	});
	running.session().send(Command::SearchMembers {
		guild_id: search_guild,
		query: "a".to_owned(),
	});
	running.session().send(Command::SearchMembers {
		guild_id: search_guild,
		query: "ab".to_owned(),
	});
	running
		.until_wire("both batches", |recording| {
			let frames = member_request_frames(recording);
			(frames
				.iter()
				.filter(|frame| frame.data["user_ids"].is_array())
				.count() == 2)
				.then_some(())
		})
		.await;
	let newcomer = fake_ids(500, 1);
	let mut again = wanted.clone();
	again.extend(newcomer.iter().copied());
	running.session().send(Command::RequestMembers {
		guild_id,
		user_ids: again,
	});
	let frames = running
		.until_wire("the request for the new id", |recording| {
			let frames = member_request_frames(recording);
			frames
				.iter()
				.any(|frame| requested_ids(frame) == [newcomer[0].0])
				.then_some(frames)
		})
		.await;
	let (by_id, searches): (Vec<&GatewayFrame>, Vec<&GatewayFrame>) =
		frames.iter().partition(|frame| frame.data["user_ids"].is_array());
	assert_eq!(by_id.len(), 3);
	assert_eq!(requested_ids(by_id[0]).len(), 100);
	assert_eq!(requested_ids(by_id[1]).len(), 50);
	let mut sent: Vec<u64> = by_id[..2].iter().flat_map(|frame| requested_ids(frame)).collect();
	sent.sort_unstable();
	let mut expected: Vec<u64> = wanted.iter().map(|id| id.0).collect();
	expected.sort_unstable();
	assert_eq!(sent, expected);
	for frame in &by_id {
		assert_eq!(frame.data["guild_id"], json!([guild_id.to_string()]));
		assert_eq!(frame.data["presences"], json!(false));
		assert!(
			frame.data["nonce"]
				.as_str()
				.is_some_and(|nonce| nonce.len() <= 32)
		);
	}
	assert!(by_id[1].at >= by_id[0].at);
	assert_eq!(searches.len(), 1);
	assert_eq!(searches[0].data["query"], json!("ab"));
	assert_eq!(searches[0].data["guild_id"], json!([search_guild.to_string()]));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rate_limited_member_request_waits_for_retry_after_then_retries() {
	let running = Running::start_with(
		Options::new("member-requests-limited")
			.prepare(|mock| mock.rate_limit_member_requests(RETRY_AFTER, 1)),
	)
	.await;
	running.connected().await;
	let guild_id = guild_ids(&running)[0];
	let members: Vec<Snowflake> = running.fixtures().guilds[0]
		.members
		.iter()
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.take(3)
		.collect();
	running.session().send(Command::RequestMembers {
		guild_id,
		user_ids: members,
	});
	let frames = running
		.until_wire("the retried request", |recording| {
			let frames = member_request_frames(recording);
			(frames.len() >= 2).then_some(frames)
		})
		.await;
	assert_eq!(requested_ids(&frames[0]), requested_ids(&frames[1]));
	assert_ne!(frames[0].data["nonce"], frames[1].data["nonce"]);
	assert!(frames[1].at.duration_since(frames[0].at) >= Duration::from_secs_f64(RETRY_AFTER));
	running.stop().await;
}
