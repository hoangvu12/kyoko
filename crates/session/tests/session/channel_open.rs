use std::time::Duration;

use mockcord::{Recording, Reply, RestRequest};
use protocol::{ChannelType, Opcode, Snowflake};
use session::Command;
use state::{ConnectionStatus, MessageRow, State};

use crate::harness::Running;

const SETTLE: Duration = Duration::from_millis(1200);

fn text_channels(state: &State) -> (Snowflake, Vec<Snowflake>) {
	let guild = state.guilds().next().expect("a guild in the fixtures");
	let mut channels: Vec<(i64, Snowflake)> = guild
		.channels
		.values()
		.filter(|channel| channel.kind == ChannelType::GuildText)
		.map(|channel| (channel.position.unwrap_or(0), channel.id))
		.collect();
	channels.sort_unstable();
	(guild.id, channels.into_iter().map(|(_, id)| id).collect())
}

fn routes_after(recording: &Recording, seen: usize) -> Vec<String> {
	recording.requests[seen..]
		.iter()
		.map(|request| match &request.query {
			Some(query) => format!("{} {}?{query}", request.method, request.path),
			None => format!("{} {}", request.method, request.path),
		})
		.collect()
}

fn history_pages(requests: &[RestRequest], channel_id: Snowflake) -> usize {
	let path = format!("/channels/{channel_id}/messages");
	requests
		.iter()
		.filter(|request| request.method == "GET" && request.path == path)
		.count()
}

fn subscriptions(recording: &Recording) -> usize {
	recording.frames_with(Opcode::GuildSubscriptionsBulk).count()
}

async fn quiet(running: &Running) -> (usize, usize) {
	tokio::time::sleep(SETTLE).await;
	running
		.mock()
		.inspect(|recording| (recording.requests.len(), subscriptions(recording)))
}

async fn open(running: &Running, guild_id: Snowflake, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the channel's first page", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
}

fn assert_only_history_and_acks(routes: &[String], channel_id: Snowflake) {
	let page = format!("GET /channels/{channel_id}/messages?limit=50");
	for route in routes {
		let ack = route.starts_with("POST /channels/") && route.ends_with("/ack");
		assert!(
			*route == page || ack,
			"a channel open sent {route} (all: {routes:?})"
		);
	}
	assert_eq!(
		routes.iter().filter(|route| **route == page).count(),
		1,
		"one history page per open: {routes:?}"
	);
	assert!(
		!routes.iter().any(|route| route.contains("/users/")),
		"no profile or user fetches on open: {routes:?}"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_channel_open_sends_one_page_and_one_subscription_and_nothing_else() {
	let running = Running::start("channel-open-requests").await;
	running.connected().await;
	let (guild_id, channels) = text_channels(&running.session().state().read());
	let (first, second) = (channels[0], channels[1]);

	let (requests, frames) = quiet(&running).await;
	open(&running, guild_id, first).await;
	let (after_first, frames_first) = quiet(&running).await;
	let routes = running
		.mock()
		.inspect(|recording| routes_after(recording, requests));
	assert_only_history_and_acks(&routes, first);
	assert_eq!(
		frames_first - frames,
		1,
		"one guild subscription for the first open"
	);

	open(&running, guild_id, second).await;
	let (after_second, frames_second) = quiet(&running).await;
	let routes = running
		.mock()
		.inspect(|recording| routes_after(recording, after_first));
	assert_only_history_and_acks(&routes, second);
	assert_eq!(
		frames_second - frames_first,
		1,
		"the second channel joins the same guild subscription"
	);

	open(&running, guild_id, first).await;
	let (_, frames_back) = quiet(&running).await;
	let back = running
		.mock()
		.inspect(|recording| history_pages(&recording.requests[after_second..], first));
	assert_eq!(back, 0, "a channel already in memory is not fetched again");
	assert!(frames_back - frames_second <= 1);
	assert_eq!(
		running.session().state().read().session().connection,
		ConnectionStatus::Connected,
		"opening channels never touches the gateway connection"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_first_page_shows_in_the_pane_only_and_retry_loads_it() {
	let running = Running::start("channel-open-failure").await;
	running.connected().await;
	let (guild_id, channels) = text_channels(&running.session().state().read());
	let channel_id = channels[0];
	running.mock().respond_once(
		"GET",
		&format!("/channels/{channel_id}/messages"),
		Reply::error(500, 0, "500: Internal Server Error"),
	);
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the pane to show the failure", |state| {
			state.history_failure(channel_id).is_some()
		})
		.await;
	{
		let state = running.session().state().read();
		assert!(matches!(
			state.channel_rows(channel_id).as_slice(),
			[MessageRow::Failed(_)]
		));
		assert_eq!(state.session().connection, ConnectionStatus::Connected);
	}
	running
		.until("the member list to load anyway", |state| {
			state
				.member_list(guild_id)
				.is_some_and(|list| list.row_count() > 0)
		})
		.await;

	running.session().send(Command::RetryHistory { channel_id });
	running
		.until("the retried page", |state| {
			state.history_failure(channel_id).is_none()
				&& state
					.messages(channel_id)
					.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
	let pages = running
		.mock()
		.inspect(|recording| history_pages(&recording.requests, channel_id));
	assert_eq!(pages, 2, "the failed page and the retry");
	running.stop().await;
}
