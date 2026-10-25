use std::time::{Duration, Instant};

use mockcord::fixtures::CURRENT_USER_ID;
use mockcord::{Recording, Reply};
use protocol::{ChannelType, Snowflake};
use session::{Command, RestTuning};
use state::{DeleteBatch, DeleteStatus, SendFailure, State};

use crate::harness::{Options, Running};
use crate::reads::{someone_else, spaced_from};

const DELETE_GAP: Duration = Duration::from_millis(300);
const OWN_MESSAGES: usize = 4;

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		delete_gap: DELETE_GAP,
		..RestTuning::default()
	}
}

fn deletes_in(recording: &Recording, channel_id: Snowflake) -> Vec<(Snowflake, Instant)> {
	let mut deletes: Vec<(Snowflake, Instant)> = recording
		.requests_to("DELETE", &format!("/channels/{channel_id}/messages/*"))
		.filter_map(|request| {
			let message_id = request.path.split('/').nth(4)?.parse().ok()?;
			Some((Snowflake(message_id), request.at))
		})
		.collect();
	deletes.sort_by_key(|(_, at)| *at);
	deletes
}

fn bulk_deletes(recording: &Recording) -> usize {
	recording
		.requests_to("POST", "/channels/*/messages/bulk-delete")
		.count()
}

fn latest_batch(state: &State) -> Option<DeleteBatch> {
	state.deletions().next_back().cloned()
}

fn settled(batch: &DeleteBatch) -> bool {
	batch
		.messages
		.iter()
		.all(|(_, status)| *status != DeleteStatus::Queued)
}

async fn finished_batch(running: &Running) -> DeleteBatch {
	running
		.until("the delete batch to finish", |state| {
			latest_batch(state).is_some_and(|batch| settled(&batch))
		})
		.await;
	latest_batch(&running.session().state().read()).expect("a delete batch")
}

async fn post_cached(running: &Running, channel_id: Snowflake, author: u64, count: usize) -> Vec<Snowflake> {
	let ids: Vec<Snowflake> = (0..count)
		.map(|index| {
			let id = running
				.mock()
				.post_message(channel_id.get(), author, &format!("mine {index}"))
				.expect("mockcord posts the message");
			Snowflake(id)
		})
		.collect();
	running
		.until("the posted messages to arrive", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| ids.iter().all(|id| cache.by_id.contains_key(id)))
		})
		.await;
	ids
}

#[tokio::test(flavor = "multi_thread")]
async fn own_messages_are_deleted_one_at_a_time() {
	let running = Running::start_with(Options::new("delete-own").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let ids = post_cached(&running, channel_id, CURRENT_USER_ID, OWN_MESSAGES).await;
	let began = Instant::now();
	running.session().send(Command::DeleteMessages {
		channel_id,
		ids: ids.clone(),
	});
	let batch = finished_batch(&running).await;
	assert!(
		batch
			.messages
			.iter()
			.all(|(_, status)| *status == DeleteStatus::Deleted),
		"{batch:?}"
	);
	let recording = running.mock().recording();
	let deletes = deletes_in(&recording, channel_id);
	assert_eq!(
		deletes.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
		ids,
		"one single delete per message, in order"
	);
	assert!(
		spaced_from(began, deletes.iter().map(|(_, at)| *at), DELETE_GAP),
		"deletes are paced"
	);
	assert_eq!(
		bulk_deletes(&recording),
		0,
		"the bot-only bulk endpoint is never called"
	);
	running
		.until("the deleted messages to leave the timeline", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| ids.iter().all(|id| !cache.by_id.contains_key(id)))
		})
		.await;
	running.stop().await;
}

async fn stops_after(name: &'static str, reply: Reply, times: usize, expected: fn(&SendFailure) -> bool) {
	let running = Running::start_with(Options::new(name).rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let ids = post_cached(&running, channel_id, CURRENT_USER_ID, OWN_MESSAGES).await;
	running.mock().respond_times(
		"DELETE",
		&format!("/channels/{channel_id}/messages/{}", ids[1]),
		reply,
		times,
	);
	running.session().send(Command::DeleteMessages {
		channel_id,
		ids: ids.clone(),
	});
	let batch = finished_batch(&running).await;
	assert_eq!(batch.status(ids[0]), Some(&DeleteStatus::Deleted));
	assert!(
		matches!(batch.status(ids[1]), Some(DeleteStatus::Failed(failure)) if expected(failure)),
		"{batch:?}"
	);
	for id in &ids[2..] {
		assert_eq!(batch.status(*id), Some(&DeleteStatus::Cancelled));
	}
	let recording = running.mock().recording();
	let attempted: Vec<Snowflake> = deletes_in(&recording, channel_id)
		.into_iter()
		.map(|(id, _)| id)
		.collect();
	assert!(
		attempted.iter().all(|id| ids[..2].contains(id)),
		"nothing after the failure is sent: {attempted:?}"
	);
	assert_eq!(bulk_deletes(&recording), 0);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_stops_at_a_forbidden_reply() {
	stops_after("delete-forbidden", Reply::forbidden(), 1, |failure| {
		*failure == SendFailure::Forbidden
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_stops_at_a_rate_limit() {
	stops_after("delete-limited", Reply::rate_limited(0.1, false), 2, |failure| {
		matches!(failure, SendFailure::RateLimited { .. })
	})
	.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn others_messages_in_a_dm_are_refused() {
	let running = Running::start_with(Options::new("delete-refused").rest(tuning())).await;
	running.connected().await;
	let dm = running
		.session()
		.state()
		.read()
		.private_channels()
		.find(|channel| channel.kind == ChannelType::Dm)
		.map(|channel| channel.id)
		.expect("a dm in the fixtures");
	running.session().send(Command::SelectChannel {
		guild_id: None,
		channel_id: dm,
	});
	running
		.until("the dm history to load", |state| state.messages(dm).is_some())
		.await;
	let theirs = post_cached(&running, dm, someone_else(&running), 1).await[0];
	let mine = post_cached(&running, dm, CURRENT_USER_ID, 1).await[0];
	running.session().send(Command::DeleteMessages {
		channel_id: dm,
		ids: vec![theirs, mine],
	});
	let batch = finished_batch(&running).await;
	assert_eq!(batch.status(theirs), Some(&DeleteStatus::Refused));
	assert_eq!(batch.status(mine), Some(&DeleteStatus::Deleted));
	let attempted: Vec<Snowflake> = deletes_in(&running.mock().recording(), dm)
		.into_iter()
		.map(|(id, _)| id)
		.collect();
	assert_eq!(attempted, vec![mine], "only the user's own message is deleted");
	assert!(running.session().state().read().last_rejection().is_some());
	running.stop().await;
}
