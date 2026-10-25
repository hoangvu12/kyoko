use std::time::{Duration, Instant};

use mockcord::{Recording, Reply, RestRequest};
use protocol::{ChannelType, Snowflake};
use session::{Command, RestTuning};
use state::{ConnectionStatus, DisconnectReason, SendFailure, State};

use crate::harness::{Options, Running};

const REACTIONS: &str = "/channels/*/messages/*/reactions/*/@me";
const EMOJI: [&str; 5] = ["👍", "❤️", "😂", "🎉", "👀"];
const RETRY_AFTER: f64 = 0.3;
const GLOBAL_RETRY_AFTER: f64 = 1.0;

fn unpaced() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn send(running: &Running, channel_id: Snowflake, content: &str) {
	running.session().send(Command::SendMessage {
		channel_id,
		content: content.to_owned(),
		reply_to: None,
		files: Vec::new(),
	});
}

fn react(running: &Running, channel_id: Snowflake, message_id: Snowflake, emoji: &str) {
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: emoji.to_owned(),
	});
}

fn messages_path(channel_id: Snowflake) -> String {
	format!("/channels/{channel_id}/messages")
}

fn posts_to(recording: &Recording, channel_id: Snowflake) -> Vec<RestRequest> {
	recording
		.requests_to("POST", &messages_path(channel_id))
		.cloned()
		.collect()
}

fn posts_of(recording: &Recording, content: &str) -> usize {
	recording
		.requests_to("POST", "/channels/*/messages")
		.filter(|request| {
			request
				.json()
				.is_some_and(|body| body["content"].as_str() == Some(content))
		})
		.count()
}

fn arrivals(requests: impl Iterator<Item = RestRequest>) -> Vec<Instant> {
	let mut arrivals: Vec<Instant> = requests.map(|request| request.at).collect();
	arrivals.sort();
	arrivals
}

fn failure(state: &State, channel_id: Snowflake, content: &str) -> Option<SendFailure> {
	state
		.messages(channel_id)?
		.pending
		.iter()
		.find(|pending| pending.message.content == content)?
		.failure
		.clone()
}

async fn failed_with(running: &Running, channel_id: Snowflake, content: &str) -> SendFailure {
	running
		.until("the send to fail", |state| {
			failure(state, channel_id, content).is_some()
		})
		.await;
	failure(&running.session().state().read(), channel_id, content).expect("a failure reason")
}

async fn delivered(running: &Running, channel_id: Snowflake, content: &str) {
	running
		.until("the message to arrive", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.ordered.iter().any(|message| message.content == content))
		})
		.await;
}

fn sibling_text_channel(state: &State, channel_id: Snowflake) -> Snowflake {
	state
		.guilds()
		.flat_map(|guild| guild.channels.values())
		.find(|channel| channel.kind == ChannelType::GuildText && channel.id != channel_id)
		.map(|channel| channel.id)
		.expect("a second text channel")
}

fn newest_message(state: &State, channel_id: Snowflake) -> Snowflake {
	state
		.messages(channel_id)
		.and_then(|cache| cache.ordered.last())
		.map(|message| message.id)
		.expect("a loaded message")
}

async fn two_channels(running: &Running) -> (Snowflake, Snowflake) {
	let first = running.open_text_channel().await;
	let second = sibling_text_channel(&running.session().state().read(), first);
	(first, second)
}

fn assert_spaced(arrivals: &[Instant], since: Instant, gap: Duration, what: &str) {
	for (index, at) in arrivals.iter().enumerate() {
		assert!(
			at.duration_since(since) >= gap * index as u32,
			"{what} {index} came too soon"
		);
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn buckets_learned_from_headers_hold_requests_until_reset() {
	let window = Duration::from_millis(600);
	let running = Running::start_with(Options::new("rest-bucket").rest(unpaced()).buckets(2, window)).await;
	let channel_id = running.open_text_channel().await;
	let message_id = newest_message(&running.session().state().read(), channel_id);
	for emoji in EMOJI {
		react(&running, channel_id, message_id, emoji);
	}
	let reactions = running
		.until_wire("every reaction", |recording| {
			let reactions: Vec<RestRequest> = recording.requests_to("PUT", REACTIONS).cloned().collect();
			(reactions.len() == EMOJI.len()).then_some(reactions)
		})
		.await;
	assert!(reactions.iter().all(|request| request.status == 204));
	let at = arrivals(reactions.into_iter());
	assert!(at[2].duration_since(at[0]) >= window);
	assert!(at[4].duration_since(at[2]) >= window);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rate_limited_send_waits_retries_once_then_reports_the_wait() {
	let running = Running::start_with(Options::new("rest-429").rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	running.mock().respond_times(
		"POST",
		&messages_path(channel_id),
		Reply::rate_limited(RETRY_AFTER, false),
		2,
	);
	send(&running, channel_id, "slow down");
	let reason = failed_with(&running, channel_id, "slow down").await;
	assert_eq!(
		reason,
		SendFailure::RateLimited {
			retry_after: Duration::from_secs_f64(RETRY_AFTER)
		}
	);
	send(&running, channel_id, "after the wait");
	delivered(&running, channel_id, "after the wait").await;
	let recording = running.mock().recording();
	let posts = posts_to(&recording, channel_id);
	assert_eq!(posts.len(), 3);
	assert!(posts[..2].iter().all(|request| request.status == 429));
	assert!(posts[1].at.duration_since(posts[0].at) >= Duration::from_secs_f64(RETRY_AFTER));
	assert!(posts[2].at.duration_since(posts[1].at) >= Duration::from_secs_f64(RETRY_AFTER));
	assert_eq!(posts_of(&recording, "slow down"), 2);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_global_rate_limit_pauses_every_route() {
	let tuning = RestTuning {
		mutation_gap: Duration::from_millis(500),
		..unpaced()
	};
	let running = Running::start_with(Options::new("rest-global").rest(tuning)).await;
	let (channel_id, other_channel) = two_channels(&running).await;
	let message_id = newest_message(&running.session().state().read(), channel_id);
	running
		.mock()
		.respond_once("PUT", REACTIONS, Reply::rate_limited(GLOBAL_RETRY_AFTER, true));
	react(&running, channel_id, message_id, EMOJI[0]);
	running
		.until_wire("the limited reaction", |recording| {
			recording.requests_to("PUT", REACTIONS).next().map(drop)
		})
		.await;
	running.session().send(Command::Typing {
		channel_id: other_channel,
	});
	let typing_path = format!("/channels/{other_channel}/typing");
	let (limited, retried, typing) = running
		.until_wire("the typing and the retried reaction", |recording| {
			let reactions: Vec<&RestRequest> = recording.requests_to("PUT", REACTIONS).collect();
			let typing = recording.requests_to("POST", &typing_path).next()?;
			(reactions.len() == 2).then(|| (reactions[0].clone(), reactions[1].clone(), typing.clone()))
		})
		.await;
	let pause = Duration::from_secs_f64(GLOBAL_RETRY_AFTER);
	assert_eq!(limited.status, 429);
	assert_eq!(retried.status, 204);
	assert!(retried.at.duration_since(limited.at) >= pause);
	assert!(typing.at.duration_since(limited.at) >= pause);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mutations_and_sends_are_paced() {
	let tuning = RestTuning {
		mutation_gap: Duration::from_millis(100),
		send_gap: Duration::from_millis(200),
		channel_send_gap: Duration::from_millis(400),
		..RestTuning::default()
	};
	let running = Running::start_with(Options::new("rest-pacing").rest(tuning)).await;
	let (channel_id, other_channel) = two_channels(&running).await;
	let message_id = newest_message(&running.session().state().read(), channel_id);
	let began = Instant::now();
	let sends = [
		(channel_id, "paced one"),
		(channel_id, "paced two"),
		(channel_id, "paced three"),
		(other_channel, "paced four"),
		(other_channel, "paced five"),
	];
	for (channel, content) in sends {
		send(&running, channel, content);
	}
	for emoji in &EMOJI[..3] {
		react(&running, channel_id, message_id, emoji);
	}
	for (channel, content) in sends {
		delivered(&running, channel, content).await;
	}
	let recording = running
		.until_wire("every reaction", |recording| {
			(recording.requests_to("PUT", REACTIONS).count() == 3).then(|| recording.clone())
		})
		.await;
	let after = |request: &&RestRequest| request.at >= began;
	let mutations = arrivals(
		recording
			.requests_to("POST", "/channels/*/messages")
			.chain(recording.requests_to("PUT", REACTIONS))
			.filter(after)
			.cloned(),
	);
	assert_eq!(mutations.len(), sends.len() + 3);
	assert_spaced(&mutations, began, tuning.mutation_gap, "mutation");
	let all_sends = arrivals(
		recording
			.requests_to("POST", "/channels/*/messages")
			.filter(after)
			.cloned(),
	);
	assert_eq!(all_sends.len(), sends.len());
	assert_spaced(&all_sends, began, tuning.send_gap, "send");
	let channel_sends = arrivals(
		posts_to(&recording, channel_id)
			.into_iter()
			.filter(|request| request.at >= began),
	);
	assert_eq!(channel_sends.len(), 3);
	assert_spaced(&channel_sends, began, tuning.channel_send_gap, "channel send");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_token_stops_every_request_and_signs_out() {
	let tuning = RestTuning {
		channel_send_gap: Duration::from_secs(1),
		..unpaced()
	};
	let running = Running::start_with(Options::new("rest-401").rest(tuning)).await;
	let (channel_id, other_channel) = two_channels(&running).await;
	let connection = running.identified_connection().await;
	running
		.mock()
		.respond_once("POST", &messages_path(channel_id), Reply::unauthorized());
	send(&running, channel_id, "rejected");
	send(&running, channel_id, "queued behind");
	assert_eq!(
		failed_with(&running, channel_id, "rejected").await,
		SendFailure::Unauthorized
	);
	assert_eq!(
		failed_with(&running, channel_id, "queued behind").await,
		SendFailure::Unauthorized
	);
	running
		.until("the signed-out status", |state| {
			state.session().connection
				== ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed)
		})
		.await;
	running
		.until_wire("the gateway to close", |recording| {
			recording
				.closes
				.iter()
				.any(|close| close.connection == connection)
				.then_some(())
		})
		.await;
	send(&running, other_channel, "after sign-out");
	running.session().send(Command::LoadOlder { channel_id });
	assert_eq!(
		failed_with(&running, other_channel, "after sign-out").await,
		SendFailure::Unauthorized
	);
	let recording = running.mock().recording();
	let rejected = recording
		.requests
		.iter()
		.find(|request| request.status == 401)
		.expect("the rejected request")
		.at;
	assert!(recording.requests.iter().all(|request| request.at <= rejected));
	assert_eq!(posts_to(&recording, channel_id).len(), 1);
	assert_eq!(recording.connections.len(), 1);
	assert_eq!(
		running.session().state().read().session().connection,
		ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_forbidden_sends_trip_a_breaker_for_that_route() {
	let tuning = RestTuning {
		forbidden_strikes: 3,
		breaker_cool_down: Duration::from_millis(400),
		..unpaced()
	};
	let running = Running::start_with(Options::new("rest-403").rest(tuning)).await;
	let (channel_id, other_channel) = two_channels(&running).await;
	running
		.mock()
		.respond_always("POST", &messages_path(channel_id), Reply::missing_permissions());
	let attempts = ["no one", "no two", "no three", "no four", "no five"];
	for content in attempts {
		send(&running, channel_id, content);
	}
	for content in attempts {
		assert_eq!(
			failed_with(&running, channel_id, content).await,
			SendFailure::MissingPermissions
		);
	}
	assert_eq!(posts_to(&running.mock().recording(), channel_id).len(), 3);
	send(&running, other_channel, "elsewhere");
	delivered(&running, other_channel, "elsewhere").await;
	let history = messages_path(channel_id);
	let pages_before = running.mock().recording().requests_to("GET", &history).count();
	running.session().send(Command::LoadOlder { channel_id });
	running
		.until_wire("older history on the same channel", |recording| {
			(recording.requests_to("GET", &history).count() > pages_before).then_some(())
		})
		.await;
	tokio::time::sleep(tuning.breaker_cool_down).await;
	running.mock().clear_replies();
	send(&running, channel_id, "allowed again");
	delivered(&running, channel_id, "allowed again").await;
	assert_eq!(posts_to(&running.mock().recording(), channel_id).len(), 4);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_captcha_is_reported_and_never_retried() {
	let running = Running::start_with(Options::new("rest-captcha").rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	running
		.mock()
		.respond_once("POST", &messages_path(channel_id), Reply::captcha_required());
	send(&running, channel_id, "prove it");
	assert_eq!(
		failed_with(&running, channel_id, "prove it").await,
		SendFailure::Captcha {
			service: Some("hcaptcha".to_owned())
		}
	);
	send(&running, channel_id, "next one");
	delivered(&running, channel_id, "next one").await;
	let recording = running.mock().recording();
	assert_eq!(posts_of(&recording, "prove it"), 1);
	assert_eq!(posts_to(&recording, channel_id).len(), 2);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_sends_carry_the_refusal_reason() {
	let running = Running::start_with(Options::new("rest-reasons").rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	let path = messages_path(channel_id);
	running.mock().respond_once("POST", &path, Reply::forbidden());
	send(&running, channel_id, "no access");
	assert_eq!(
		failed_with(&running, channel_id, "no access").await,
		SendFailure::Forbidden
	);
	running
		.mock()
		.respond_once("POST", &path, Reply::error(500, 0, "Internal Server Error"));
	send(&running, channel_id, "server trouble");
	assert_eq!(
		failed_with(&running, channel_id, "server trouble").await,
		SendFailure::Other(Some(500))
	);
	let recording = running.mock().recording();
	assert_eq!(posts_of(&recording, "no access"), 1);
	assert_eq!(posts_of(&recording, "server trouble"), 1);
	running.stop().await;
}
