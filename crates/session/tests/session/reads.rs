use std::collections::HashSet;
use std::time::{Duration, Instant};

use mockcord::Recording;
use mockcord::fixtures::CURRENT_USER_ID;
use protocol::{ChannelType, Permissions, Snowflake};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const QUIET: Duration = Duration::from_millis(300);
const BULK_GAP: Duration = Duration::from_millis(400);
const BURST: usize = 5;
const BURST_SPACING: Duration = Duration::from_millis(60);
const ACK_BULK_LIMIT: usize = 100;
const MANY_CHANNELS: usize = 70;

pub fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		ack_delay: QUIET,
		ack_jitter: Duration::ZERO,
		ack_max_delay: Duration::from_secs(5),
		bulk_ack_gap: BULK_GAP,
		..RestTuning::default()
	}
}

pub fn someone_else(running: &Running) -> u64 {
	*running
		.fixtures()
		.users
		.keys()
		.find(|id| **id != CURRENT_USER_ID)
		.expect("another user in the fixtures")
}

fn acks_to(recording: &Recording, channel_id: Snowflake) -> Vec<(Snowflake, Instant)> {
	recording
		.requests_to("POST", &format!("/channels/{channel_id}/messages/*/ack"))
		.filter_map(|request| {
			let message_id = request.path.split('/').nth(4)?.parse().ok()?;
			Some((Snowflake(message_id), request.at))
		})
		.collect()
}

fn acks_from(recording: &Recording, channel_id: Snowflake, first: Snowflake) -> Vec<(Snowflake, Instant)> {
	acks_to(recording, channel_id)
		.into_iter()
		.filter(|(message_id, _)| *message_id >= first)
		.collect()
}

async fn ack_of(running: &Running, channel_id: Snowflake, message_id: Snowflake) -> Instant {
	running
		.until_wire("the ack", |recording| {
			acks_to(recording, channel_id)
				.into_iter()
				.find(|(acked, _)| *acked == message_id)
				.map(|(_, at)| at)
		})
		.await
}

fn post(running: &Running, channel_id: Snowflake, author: u64, content: &str) -> Snowflake {
	Snowflake(
		running
			.mock()
			.post_message(channel_id.get(), author, content)
			.expect("mockcord posts the message"),
	)
}

async fn settle() {
	tokio::time::sleep(QUIET * 3).await;
}

fn other_text_channel(state: &State, open: Snowflake) -> Snowflake {
	state
		.guilds()
		.flat_map(|guild| guild.channels.values())
		.find(|channel| channel.kind == ChannelType::GuildText && channel.id != open)
		.map(|channel| channel.id)
		.expect("a second text channel")
}

pub fn spaced_from(began: Instant, arrivals: impl Iterator<Item = Instant>, gap: Duration) -> bool {
	let mut arrivals: Vec<Instant> = arrivals.collect();
	arrivals.sort();
	arrivals
		.iter()
		.enumerate()
		.all(|(index, at)| *at >= began + gap * index as u32)
}

fn bulk_entries(recording: &Recording) -> Vec<(Instant, Vec<Snowflake>)> {
	recording
		.requests_to("POST", "/read-states/ack-bulk")
		.map(|request| {
			let body = request.json().expect("a json body");
			let channels = body["read_states"]
				.as_array()
				.expect("read states")
				.iter()
				.filter_map(|entry| entry["channel_id"].as_str()?.parse().ok().map(Snowflake))
				.collect();
			(request.at, channels)
		})
		.collect()
}

fn unread_channels(state: &State) -> HashSet<Snowflake> {
	let guild_channels = state.guilds().flat_map(|guild| guild.channels.values());
	guild_channels
		.filter(|channel| {
			state
				.my_permissions(channel.id)
				.contains(Permissions::VIEW_CHANNEL)
		})
		.chain(state.private_channels())
		.filter(|channel| {
			let Some(read_state) = state.read_state(channel.id) else {
				return false;
			};
			channel.kind != ChannelType::GuildCategory
				&& (read_state.last_message_id < channel.last_message_id
					|| read_state.mention_count.unwrap_or(0) > 0)
		})
		.map(|channel| channel.id)
		.collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn burst_in_open_channel_is_acked_once_for_newest() {
	let running = Running::start_with(Options::new("ack-burst").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let author = someone_else(&running);
	let mut posted = Vec::new();
	let mut before_last = Instant::now();
	for index in 0..BURST {
		before_last = Instant::now();
		posted.push(post(&running, channel_id, author, &format!("burst {index}")));
		tokio::time::sleep(BURST_SPACING).await;
	}
	let newest = *posted.last().expect("a burst");
	let acked_at = ack_of(&running, channel_id, newest).await;
	assert!(
		acked_at >= before_last + QUIET,
		"the ack waits for a quiet period after the last message"
	);
	settle().await;
	let acks = acks_from(&running.mock().recording(), channel_id, posted[0]);
	assert_eq!(acks.len(), 1, "one ack for the whole burst: {acks:?}");
	running
		.until("the channel to read up to the burst", |state| {
			state
				.read_state(channel_id)
				.is_some_and(|read_state| read_state.last_message_id == Some(newest))
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn channels_that_are_not_open_are_not_acked() {
	let running = Running::start_with(Options::new("ack-closed").rest(tuning())).await;
	let open = running.open_text_channel().await;
	let closed = other_text_channel(&running.session().state().read(), open);
	let author = someone_else(&running);
	let elsewhere = post(&running, closed, author, "not looking");
	let here = post(&running, open, author, "looking");
	ack_of(&running, open, here).await;
	settle().await;
	assert!(
		acks_from(&running.mock().recording(), closed, elsewhere).is_empty(),
		"a channel the user is not viewing is never acked"
	);
	let still_unread = running
		.session()
		.state()
		.read()
		.read_state(closed)
		.is_none_or(|read_state| read_state.last_message_id < Some(elsewhere));
	assert!(still_unread, "the closed channel stays unread");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn hidden_window_defers_the_ack_until_visible() {
	let running = Running::start_with(Options::new("ack-hidden").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::SetViewing(false));
	running.session().send(Command::SetDraft {
		channel_id,
		content: "hidden".to_owned(),
	});
	running
		.until("the hidden flag to apply", |state| {
			state.draft(channel_id) == Some("hidden")
		})
		.await;
	let unseen = post(&running, channel_id, someone_else(&running), "while away");
	settle().await;
	assert!(
		acks_from(&running.mock().recording(), channel_id, unseen).is_empty(),
		"nothing is acked while the window is hidden"
	);
	let shown = Instant::now();
	running.session().send(Command::SetViewing(true));
	let acked_at = ack_of(&running, channel_id, unseen).await;
	assert!(
		acked_at >= shown + QUIET,
		"the ack still waits for the quiet period"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn mark_all_read_clears_badges_with_bulk_acks() {
	let running = Running::start_with(
		Options::new("mark-all-read")
			.channels(MANY_CHANNELS)
			.rest(tuning()),
	)
	.await;
	running.connected().await;
	let unread = unread_channels(&running.session().state().read());
	assert!(
		unread.len() > ACK_BULK_LIMIT,
		"the fixtures start with more unread channels than one request takes"
	);
	let began = Instant::now();
	running.session().send(Command::MarkAllRead);
	running
		.until("every badge to clear", |state| {
			state
				.guild_summaries()
				.iter()
				.all(|guild| !guild.unread && guild.mentions == 0)
				&& state.dm_rows().iter().all(|dm| !dm.unread && dm.mentions == 0)
		})
		.await;
	let requests = running
		.until_wire("bulk acks for every unread channel", |recording| {
			let requests = bulk_entries(recording);
			let acked: usize = requests.iter().map(|(_, channels)| channels.len()).sum();
			(acked >= unread.len()).then_some(requests)
		})
		.await;
	let acked: HashSet<Snowflake> = requests
		.iter()
		.flat_map(|(_, channels)| channels.iter().copied())
		.collect();
	assert_eq!(acked, unread, "exactly the unread channels are acked");
	assert!(
		requests
			.iter()
			.all(|(_, channels)| channels.len() <= ACK_BULK_LIMIT)
	);
	assert_eq!(requests.len(), unread.len().div_ceil(ACK_BULK_LIMIT));
	assert!(
		spaced_from(began, requests.iter().map(|(at, _)| *at), BULK_GAP),
		"bulk acks are paced"
	);
	assert_eq!(
		running
			.mock()
			.recording()
			.requests_to("POST", "/channels/*/messages/*/ack")
			.count(),
		0,
		"marking read never falls back to one ack per channel"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn marking_two_servers_read_is_paced() {
	let running = Running::start_with(Options::new("mark-guilds-read").rest(tuning())).await;
	running.connected().await;
	let guilds: Vec<Snowflake> = running
		.session()
		.state()
		.read()
		.guild_summaries()
		.iter()
		.filter(|guild| guild.unread)
		.map(|guild| guild.id)
		.take(2)
		.collect();
	assert_eq!(guilds.len(), 2, "two servers with unread channels");
	let began = Instant::now();
	for &guild_id in &guilds {
		running.session().send(Command::MarkGuildRead { guild_id });
	}
	running
		.until("both servers to read", |state| {
			guilds
				.iter()
				.all(|id| state.guild_summary(*id).is_some_and(|guild| !guild.unread))
		})
		.await;
	let requests = running
		.until_wire("one bulk ack per server", |recording| {
			let requests = bulk_entries(recording);
			(requests.len() >= 2).then_some(requests)
		})
		.await;
	assert_eq!(requests.len(), 2);
	assert!(spaced_from(began, requests.iter().map(|(at, _)| *at), BULK_GAP));
	let owners: Vec<HashSet<Option<Snowflake>>> = {
		let state = running.session().state().read();
		requests
			.iter()
			.map(|(_, channels)| {
				channels
					.iter()
					.map(|channel| state.channel_guild(*channel))
					.collect()
			})
			.collect()
	};
	assert!(
		owners.iter().all(|owners| owners.len() == 1),
		"each request covers only one server"
	);
	let covered: HashSet<Snowflake> = owners.into_iter().flatten().flatten().collect();
	assert_eq!(covered, guilds.iter().copied().collect());
	running.stop().await;
}
