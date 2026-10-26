use std::time::Duration;

use protocol::{Message, MessageFlags, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const LINK: &str = "docs are up https://github.com/refact0r/midnight-discord";
const QUIET: Duration = Duration::from_millis(400);

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn message(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Option<Message> {
	state
		.messages(channel_id)
		.and_then(|cache| cache.by_id.get(&message_id))
		.map(|message| message.as_ref().clone())
}

fn mine_with_embed(state: &State, channel_id: Snowflake) -> Option<Snowflake> {
	let me = state.current_user_id()?;
	state.messages(channel_id).and_then(|cache| {
		cache
			.ordered
			.iter()
			.rev()
			.find(|message| message.author.id == me && message.content == LINK && !message.embeds.is_empty())
			.map(|message| message.id)
	})
}

fn someone_elses(state: &State, channel_id: Snowflake) -> Option<Snowflake> {
	let me = state.current_user_id()?;
	state.messages(channel_id).and_then(|cache| {
		cache
			.ordered
			.iter()
			.rev()
			.find(|message| message.author.id != me)
			.map(|message| message.id)
	})
}

fn suppressed(state: &State, channel_id: Snowflake, message_id: Snowflake) -> bool {
	message(state, channel_id, message_id)
		.is_some_and(|message| message.flags.contains(MessageFlags::SUPPRESS_EMBEDS))
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_embeds_sets_the_suppress_flag_on_your_own_message_only() {
	let running = Running::start_with(Options::new("suppress-embeds").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::SendMessage {
		channel_id,
		content: LINK.to_owned(),
		reply_to: None,
		ping_reply: false,
		files: Vec::new(),
	});
	running
		.until("the sent link with its embed", |state| {
			mine_with_embed(state, channel_id).is_some()
		})
		.await;
	let (mine, theirs) = {
		let state = running.session().state().read();
		(
			mine_with_embed(&state, channel_id).expect("the sent link"),
			someone_elses(&state, channel_id).expect("a message from someone else"),
		)
	};
	running.session().send(Command::SuppressEmbeds {
		channel_id,
		message_id: mine,
	});
	running
		.until("the suppressed embeds to echo back", |state| {
			suppressed(state, channel_id, mine)
		})
		.await;
	let rejected_before = running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id);
	running.session().send(Command::SuppressEmbeds {
		channel_id,
		message_id: theirs,
	});
	running.session().send(Command::SuppressEmbeds {
		channel_id,
		message_id: mine,
	});
	running
		.until("the refusal for someone else's message", |state| {
			state.last_rejection().is_some_and(|rejection| {
				rejection.id > rejected_before && rejection.reason.contains("own messages")
			})
		})
		.await;
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	let edits: Vec<Value> = recording
		.requests_to("PATCH", &format!("/channels/{channel_id}/messages/{mine}"))
		.map(|request| serde_json::from_slice(&request.body).expect("a JSON body"))
		.collect();
	assert_eq!(
		edits,
		vec![json!({ "flags": MessageFlags::SUPPRESS_EMBEDS.bits() })],
		"one edit carrying only the flags; an already suppressed message is not sent again"
	);
	assert_eq!(
		recording
			.requests_to("PATCH", &format!("/channels/{channel_id}/messages/{theirs}"))
			.count(),
		0,
		"someone else's message never reaches REST"
	);
	let state = running.session().state().read();
	let kept = message(&state, channel_id, mine).expect("the message");
	assert!(
		!kept.embeds.is_empty(),
		"Discord keeps the embeds and sets the flag"
	);
	assert!(kept.edited_timestamp.is_none(), "removing embeds is not an edit");
	drop(state);
	running.stop().await;
}
