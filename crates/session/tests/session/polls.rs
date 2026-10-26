use std::time::Duration;

use mockcord::{Recording, RestRequest};
use protocol::{PartialEmoji, Poll, Snowflake};
use serde_json::{Value, json};
use session::{Command, PollDraft, PollDraftAnswer, RestTuning, RiskyReason};
use state::State;

use crate::harness::{Options, Running};

const CUSTOM_EMOJI: u64 = 1_100_000_000_000_000_042;
const SEND_GAP: Duration = Duration::from_millis(300);
const QUESTION: &str = "What should we play tonight?";
const MESSAGES: &str = "/channels/*/messages";
const VOTES: &str = "/channels/*/polls/*/answers/@me";

fn answer(text: &str, emoji: Option<PartialEmoji>) -> PollDraftAnswer {
	PollDraftAnswer {
		text: text.to_owned(),
		emoji,
	}
}

fn unicode(name: &str) -> Option<PartialEmoji> {
	Some(PartialEmoji {
		id: None,
		name: Some(name.to_owned()),
		animated: false,
	})
}

fn draft() -> PollDraft {
	PollDraft {
		question: format!("  {QUESTION} "),
		answers: vec![
			answer("Valorant", unicode("\u{1f3af}")),
			answer(
				" Minecraft ",
				Some(PartialEmoji {
					id: Some(Snowflake(CUSTOM_EMOJI)),
					name: Some("pickaxe".to_owned()),
					animated: false,
				}),
			),
			answer("   ", None),
			answer("Lethal Company", None),
		],
		duration_hours: 24,
		multiple: false,
	}
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

async fn rejected_since(running: &Running, before: u64) -> String {
	running
		.until("a rejection", |state| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.id > before)
		})
		.await;
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map(|rejection| rejection.reason.clone())
		.unwrap_or_default()
}

fn poll_posts(recording: &Recording) -> Vec<RestRequest> {
	recording
		.requests_to("POST", MESSAGES)
		.filter(|request| request.json().is_some_and(|body| body.get("poll").is_some()))
		.cloned()
		.collect()
}

fn poll_of(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Option<Poll> {
	state.messages(channel_id)?.by_id.get(&message_id)?.poll.clone()
}

fn count(poll: &Poll, answer_id: i64) -> (i64, bool) {
	poll.results
		.as_ref()
		.and_then(|results| results.answer_counts.iter().find(|count| count.id == answer_id))
		.map_or((0, false), |count| (count.count, count.me_voted))
}

fn posted_poll(state: &State, channel_id: Snowflake) -> Option<Snowflake> {
	state
		.messages(channel_id)?
		.ordered
		.iter()
		.rev()
		.find_map(|message| {
			message
				.poll
				.as_ref()
				.filter(|poll| poll.question.text.as_deref() == Some(QUESTION))
				.map(|_| message.id)
		})
}

async fn post_poll(running: &Running, channel_id: Snowflake) -> Snowflake {
	running.session().send(Command::SendPoll {
		channel_id,
		poll: draft(),
	});
	running
		.until("the poll to come back from mockcord", |state| {
			posted_poll(state, channel_id).is_some()
		})
		.await;
	posted_poll(&running.session().state().read(), channel_id).expect("the posted poll")
}

fn votes(recording: &Recording) -> Vec<Value> {
	recording
		.requests_to("PUT", VOTES)
		.filter_map(RestRequest::json)
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn creating_a_poll_posts_it_as_the_web_client_does_and_shows_it_while_pending() {
	let running = Running::start_with(Options::new("polls-create")).await;
	let channel_id = running.open_text_channel().await;
	running.mock().hold_requests("POST", MESSAGES);
	running.session().send(Command::SendPoll {
		channel_id,
		poll: draft(),
	});
	running
		.until("the pending poll", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.iter().any(|pending| {
					pending.message.poll.as_ref().is_some_and(|poll| {
						poll.answers.len() == 3 && poll.question.text.as_deref() == Some(QUESTION)
					})
				})
			})
		})
		.await;
	running.mock().release_requests();
	let posted = running
		.until_wire("the poll on the wire", |recording| poll_posts(recording).pop())
		.await;
	assert_eq!(posted.path, format!("/channels/{channel_id}/messages"));
	let body = posted.json().expect("a json body");
	let nonce = body["nonce"].as_str().expect("a nonce").to_owned();
	assert_eq!(
		body,
		json!({
			"nonce": nonce,
			"poll": {
				"question": { "text": QUESTION },
				"answers": [
					{ "poll_media": { "text": "Valorant", "emoji": { "name": "\u{1f3af}" } } },
					{ "poll_media": { "text": "Minecraft", "emoji": { "id": CUSTOM_EMOJI.to_string() } } },
					{ "poll_media": { "text": "Lethal Company" } },
				],
				"allow_multiselect": false,
				"duration": 24,
				"layout_type": 1,
			},
		})
	);
	let message_id = {
		running
			.until("the sent poll to replace the pending one", |state| {
				posted_poll(state, channel_id).is_some()
					&& state
						.messages(channel_id)
						.is_some_and(|cache| cache.pending.is_empty())
			})
			.await;
		posted_poll(&running.session().state().read(), channel_id).expect("the poll")
	};
	let poll = poll_of(&running.session().state().read(), channel_id, message_id).expect("a poll");
	let ids: Vec<Option<i64>> = poll.answers.iter().map(|answer| answer.answer_id).collect();
	assert_eq!(ids, [Some(1), Some(2), Some(3)]);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn limits_are_refused_before_anything_is_sent() {
	let running = Running::start_with(Options::new("polls-limits")).await;
	let channel_id = running.open_text_channel().await;
	let too_many = PollDraft {
		answers: (0..11)
			.map(|index| answer(&format!("answer {index}"), None))
			.collect(),
		..draft()
	};
	let cases = [
		(
			PollDraft {
				question: "   ".to_owned(),
				..draft()
			},
			"needs a question",
		),
		(
			PollDraft {
				question: "q".repeat(301),
				..draft()
			},
			"at most 300 characters",
		),
		(too_many, "at most 10 answers"),
		(
			PollDraft {
				answers: vec![answer("", unicode("\u{1f3af}")), answer(" ", None)],
				..draft()
			},
			"at least one answer",
		),
		(
			PollDraft {
				answers: vec![answer(&"a".repeat(56), None)],
				..draft()
			},
			"at most 55 characters",
		),
		(
			PollDraft {
				duration_hours: 5,
				..draft()
			},
			"durations",
		),
	];
	for (poll, words) in cases {
		let before = rejections(&running);
		running.session().send(Command::SendPoll { channel_id, poll });
		let reason = rejected_since(&running, before).await;
		assert!(reason.contains(words), "{reason:?} should say {words:?}");
	}
	let longest = PollDraft {
		question: "q".repeat(300),
		answers: (0..10).map(|_| answer(&"a".repeat(55), None)).collect(),
		duration_hours: 336,
		multiple: true,
	};
	running.session().send(Command::SendPoll {
		channel_id,
		poll: longest,
	});
	let sent = running
		.until_wire("the longest poll allowed", |recording| {
			poll_posts(recording).pop()
		})
		.await;
	let body = sent.json().expect("a json body");
	assert_eq!(body["poll"]["answers"].as_array().map(Vec::len), Some(10));
	assert_eq!(body["poll"]["duration"], json!(336));
	assert_eq!(body["poll"]["allow_multiselect"], json!(true));
	assert_eq!(poll_posts(&running.mock().recording()).len(), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn voting_and_removing_a_vote_put_the_answers_and_the_counts_follow_the_gateway() {
	let running = Running::start_with(Options::new("polls-vote")).await;
	let channel_id = running.open_text_channel().await;
	let message_id = post_poll(&running, channel_id).await;
	running.session().send(Command::VotePoll {
		channel_id,
		message_id,
		answer_ids: vec![2],
	});
	running
		.until("the vote to count", |state| {
			poll_of(state, channel_id, message_id).is_some_and(|poll| count(&poll, 2) == (1, true))
		})
		.await;
	running.session().send(Command::VotePoll {
		channel_id,
		message_id,
		answer_ids: vec![1],
	});
	running
		.until("the vote to move", |state| {
			poll_of(state, channel_id, message_id)
				.is_some_and(|poll| count(&poll, 1) == (1, true) && count(&poll, 2) == (0, false))
		})
		.await;
	running.session().send(Command::VotePoll {
		channel_id,
		message_id,
		answer_ids: Vec::new(),
	});
	running
		.until("the vote to be removed", |state| {
			poll_of(state, channel_id, message_id).is_some_and(|poll| count(&poll, 1) == (0, false))
		})
		.await;
	let sent = votes(&running.mock().recording());
	assert_eq!(
		sent,
		[
			json!({ "answer_ids": [2] }),
			json!({ "answer_ids": [1] }),
			json!({ "answer_ids": [] }),
		]
	);
	let paths: Vec<String> = running
		.mock()
		.recording()
		.requests_to("PUT", VOTES)
		.map(|request| request.path.clone())
		.collect();
	assert!(
		paths
			.iter()
			.all(|path| *path == format!("/channels/{channel_id}/polls/{message_id}/answers/@me"))
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn votes_from_others_update_the_counts_live() {
	let running = Running::start_with(Options::new("polls-live")).await;
	let channel_id = running.open_text_channel().await;
	let message_id = post_poll(&running, channel_id).await;
	let me = running.fixtures().current_user.id;
	let others: Vec<u64> = running
		.fixtures()
		.users
		.keys()
		.copied()
		.filter(|id| *id != me.get())
		.take(2)
		.collect();
	for other in &others {
		assert!(
			running
				.mock()
				.poll_vote_as(channel_id.get(), message_id.get(), *other, 3, true)
		);
	}
	running
		.until("two votes from others", |state| {
			poll_of(state, channel_id, message_id).is_some_and(|poll| count(&poll, 3) == (2, false))
		})
		.await;
	assert!(
		running
			.mock()
			.poll_vote_as(channel_id.get(), message_id.get(), others[0], 3, false)
	);
	running
		.until("a vote taken back", |state| {
			poll_of(state, channel_id, message_id).is_some_and(|poll| count(&poll, 3) == (1, false))
		})
		.await;
	running.mock().dispatch(
		"MESSAGE_POLL_VOTE_ADD",
		&json!({
			"user_id": others[1].to_string(),
			"channel_id": channel_id.to_string(),
			"message_id": message_id.to_string(),
			"answer_id": "not a number",
			"extra": true,
		}),
	);
	assert!(
		running
			.mock()
			.poll_vote_as(channel_id.get(), message_id.get(), others[1], 1, true)
	);
	running
		.until("a vote after a bad event", |state| {
			poll_of(state, channel_id, message_id).is_some_and(|poll| count(&poll, 1) == (1, false))
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ending_a_poll_finalises_it_and_posts_the_result() {
	let running = Running::start_with(Options::new("polls-end")).await;
	let channel_id = running.open_text_channel().await;
	let message_id = post_poll(&running, channel_id).await;
	running.session().send(Command::VotePoll {
		channel_id,
		message_id,
		answer_ids: vec![1],
	});
	running
		.until("the vote", |state| {
			poll_of(state, channel_id, message_id).is_some_and(|poll| count(&poll, 1) == (1, true))
		})
		.await;
	running.session().send(Command::EndPoll {
		channel_id,
		message_id,
	});
	running
		.until("the poll to close and its result to arrive", |state| {
			let closed = poll_of(state, channel_id, message_id)
				.is_some_and(|poll| poll.results.is_some_and(|results| results.is_finalized));
			let result = state.messages(channel_id).is_some_and(|cache| {
				cache.ordered.iter().any(|message| {
					message.kind == protocol::MessageType::PollResult
						&& message
							.message_reference
							.as_ref()
							.and_then(|reference| reference.message_id)
							== Some(message_id)
				})
			});
			closed && result
		})
		.await;
	let expired = running
		.mock()
		.recording()
		.requests_to(
			"POST",
			&format!("/channels/{channel_id}/polls/{message_id}/expire"),
		)
		.count();
	assert_eq!(expired, 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn polls_are_paced_sends_and_respect_locks() {
	let tuning = RestTuning {
		send_gap: SEND_GAP,
		..RestTuning::default()
	};
	let running = Running::start_with(Options::new("polls-paced").rest(tuning).prepare(|mock| {
		mock.transform_ready(|_, ready| {
			ready["private_channels"][1]["is_spam"] = json!(true);
		});
	}))
	.await;
	let channel_id = running.open_text_channel().await;
	let spam = Snowflake(running.fixtures().private_channel_ids[1]);
	running
		.until("the spam lock", |state| {
			state
				.channel_lock(spam, chrono::Utc::now())
				.map(|lock| lock.reason)
				== Some(RiskyReason::SpamDm)
		})
		.await;
	let before = rejections(&running);
	running.session().send(Command::SendPoll {
		channel_id: spam,
		poll: draft(),
	});
	let reason = rejected_since(&running, before).await;
	assert_eq!(reason, RiskyReason::SpamDm.explanation());
	for _ in 0..3 {
		running.session().send(Command::SendPoll {
			channel_id,
			poll: draft(),
		});
	}
	let sent = running
		.until_wire("three polls", |recording| {
			let posts = poll_posts(recording);
			(posts.len() == 3).then_some(posts)
		})
		.await;
	running.assert_released_apart(&sent, SEND_GAP, "poll");
	assert!(
		running
			.mock()
			.recording()
			.requests_to("POST", &format!("/channels/{spam}/messages"))
			.next()
			.is_none()
	);
	running.stop().await;
}
