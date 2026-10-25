use std::time::Duration;

use bytes::Bytes;
use mockcord::{MOCKBOT_ID, Recording, Reply, RestRequest};
use protocol::{CommandOptionType, InteractionOption, Nonce, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning, UploadFile};
use state::{SendFailure, State};

use crate::harness::{Options, Running};

const MESSAGES: &str = "/channels/*/messages";
const ECHO_ID: Snowflake = Snowflake(9_100_000_000_000_000_102);
const BROKEN_ID: Snowflake = Snowflake(9_100_000_000_000_000_104);
type Case = (&'static str, Reply, fn(&SendFailure) -> bool);

const QUIET: Duration = Duration::from_millis(600);

fn unpaced() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

async fn start(name: &'static str) -> (Running, Snowflake) {
	let running = Running::start_with(Options::new(name).rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	(running, channel_id)
}

fn send(
	running: &Running,
	channel_id: Snowflake,
	content: &str,
	reply_to: Option<Snowflake>,
	ping_reply: bool,
) {
	running.session().send(Command::SendMessage {
		channel_id,
		content: content.to_owned(),
		reply_to,
		ping_reply,
		files: Vec::new(),
	});
}

fn post_with(recording: &Recording, content: &str) -> Option<Value> {
	recording
		.requests_to("POST", MESSAGES)
		.filter_map(RestRequest::json)
		.find(|body| body["content"].as_str() == Some(content))
}

fn posts_of(recording: &Recording, content: &str) -> usize {
	recording
		.requests_to("POST", MESSAGES)
		.filter_map(RestRequest::json)
		.filter(|body| body["content"].as_str() == Some(content))
		.count()
}

fn newest_from_others(state: &State, channel_id: Snowflake) -> Snowflake {
	let me = state.current_user_id();
	state
		.messages(channel_id)
		.and_then(|cache| {
			cache
				.ordered
				.iter()
				.rev()
				.find(|message| Some(message.author.id) != me)
		})
		.map(|message| message.id)
		.expect("a message from someone else")
}

fn pending_failure(state: &State, channel_id: Snowflake, content: &str) -> Option<(Nonce, SendFailure)> {
	let pending = state
		.messages(channel_id)?
		.pending
		.iter()
		.find(|pending| pending.message.content == content)?;
	Some((pending.message.nonce.clone()?, pending.failure.clone()?))
}

async fn delivered(running: &Running, channel_id: Snowflake, content: &str) {
	running
		.until("the message to arrive", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.ordered.iter().any(|message| message.content == content)
					&& !cache
						.pending
						.iter()
						.any(|pending| pending.message.content == content)
			})
		})
		.await;
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

async fn rejected_with(running: &Running, before: u64) -> String {
	running
		.until("the action to be refused", |state| {
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
		.expect("a rejection")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reply_without_ping_tells_discord_not_to_mention_the_author() {
	let (running, channel_id) = start("composer-reply").await;
	let target = newest_from_others(&running.session().state().read(), channel_id);
	send(&running, channel_id, "quiet reply", Some(target), false);
	send(&running, channel_id, "loud reply", Some(target), true);
	delivered(&running, channel_id, "loud reply").await;
	delivered(&running, channel_id, "quiet reply").await;
	let recording = running.mock().recording();
	let quiet = post_with(&recording, "quiet reply").expect("the quiet reply was posted");
	assert_eq!(
		quiet["message_reference"]["message_id"],
		json!(target.to_string())
	);
	assert_eq!(quiet["allowed_mentions"]["replied_user"], json!(false));
	assert_eq!(
		quiet["allowed_mentions"]["parse"],
		json!(["users", "roles", "everyone"])
	);
	let loud = post_with(&recording, "loud reply").expect("the loud reply was posted");
	assert_eq!(loud["message_reference"]["message_id"], json!(target.to_string()));
	assert!(loud.get("allowed_mentions").is_none());
	let echoed = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| {
			cache
				.ordered
				.iter()
				.find(|message| message.content == "quiet reply")
				.cloned()
		})
		.expect("the quiet reply is cached");
	assert_eq!(
		echoed.referenced_message.as_ref().map(|message| message.id),
		Some(target)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_edit_changes_the_message_and_a_refused_edit_says_why() {
	let (running, channel_id) = start("composer-edit").await;
	let message_id = running
		.send_and_wait(channel_id, "first draft", "first draft")
		.await;
	running.session().send(Command::EditMessage {
		channel_id,
		message_id,
		content: "second draft".to_owned(),
	});
	running
		.until("the edit to arrive", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&message_id))
				.is_some_and(|message| message.content == "second draft")
		})
		.await;
	let path = format!("/channels/{channel_id}/messages/{message_id}");
	running.mock().respond_once("PATCH", &path, Reply::forbidden());
	let before = rejections(&running);
	running.session().send(Command::EditMessage {
		channel_id,
		message_id,
		content: "third draft".to_owned(),
	});
	let reason = rejected_with(&running, before).await;
	assert_eq!(reason, "Edit: missing permissions");
	let recording = running.mock().recording();
	assert_eq!(recording.requests_to("PATCH", &path).count(), 2);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn files_upload_to_their_slots_before_the_message_is_posted() {
	let (running, channel_id) = start("composer-upload").await;
	let bytes = Bytes::from(vec![7u8; 4096]);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "with a file".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: vec![UploadFile {
			name: "notes.txt".to_owned(),
			bytes: bytes.clone(),
			content_type: "text/plain".to_owned(),
		}],
	});
	delivered(&running, channel_id, "with a file").await;
	let recording = running.mock().recording();
	let slots_path = format!("/channels/{channel_id}/attachments");
	let slots: Vec<&RestRequest> = recording.requests_to("POST", &slots_path).collect();
	assert_eq!(slots.len(), 1);
	let asked = slots[0].json().expect("a json slot request");
	assert_eq!(asked["files"][0]["filename"], json!("notes.txt"));
	assert_eq!(asked["files"][0]["file_size"], json!(4096));
	let uploads: Vec<&RestRequest> = recording.requests_to("PUT", "/upload/**").collect();
	assert_eq!(uploads.len(), 1);
	assert_eq!(uploads[0].body.len(), 4096);
	assert!(uploads[0].header("authorization").is_none());
	assert!(uploads[0].header("x-super-properties").is_none());
	let posted = post_with(&recording, "with a file").expect("the message was posted");
	assert_eq!(posted["attachments"][0]["filename"], json!("notes.txt"));
	assert!(
		posted["attachments"][0]["uploaded_filename"]
			.as_str()
			.is_some_and(|name| name.ends_with("/notes.txt"))
	);
	assert!(uploads[0].at < recording.requests_to("POST", MESSAGES).last().expect("a post").at);
	let message = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| {
			cache
				.ordered
				.iter()
				.find(|message| message.content == "with a file")
				.cloned()
		})
		.expect("the message is cached");
	assert_eq!(message.attachments.len(), 1);
	assert_eq!(message.attachments[0].size, Some(4096));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_file_over_the_limit_is_refused_before_any_request() {
	let options = Options::new("composer-too-large")
		.rest(unpaced())
		.prepare(|mock| mock.transform_ready(|_, ready| ready["user"]["premium_type"] = json!(0)));
	let running = Running::start_with(options).await;
	let channel_id = running.open_text_channel().await;
	let limit = running.session().state().read().upload_limit(channel_id);
	assert_eq!(limit, 10 * 1024 * 1024);
	let before = rejections(&running);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "too many".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: (0..11)
			.map(|index| UploadFile {
				name: format!("{index}.txt"),
				bytes: Bytes::from_static(b"x"),
				content_type: "text/plain".to_owned(),
			})
			.collect(),
	});
	assert_eq!(
		rejected_with(&running, before).await,
		"You can attach up to 10 files"
	);
	let before = rejections(&running);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "too big".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: vec![UploadFile {
			name: "huge.bin".to_owned(),
			bytes: Bytes::from(vec![0u8; limit as usize + 1]),
			content_type: "application/octet-stream".to_owned(),
		}],
	});
	let reason = rejected_with(&running, before).await;
	assert_eq!(reason, "huge.bin is larger than the 10 MB upload limit");
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	assert_eq!(
		recording.requests_to("POST", "/channels/*/attachments").count(),
		0
	);
	assert_eq!(posts_of(&recording, "too big"), 0);
	assert!(
		running
			.session()
			.state()
			.read()
			.messages(channel_id)
			.is_some_and(|cache| cache.pending.is_empty())
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typing_posts_at_most_once_per_window_and_again_after_a_send() {
	let (running, channel_id) = start("composer-typing").await;
	let typing = format!("/channels/{channel_id}/typing");
	for _ in 0..5 {
		running.session().send(Command::Typing { channel_id });
	}
	running
		.until_wire("the first typing post", |recording| {
			recording.requests_to("POST", &typing).next().map(drop)
		})
		.await;
	tokio::time::sleep(QUIET).await;
	assert_eq!(running.mock().recording().requests_to("POST", &typing).count(), 1);
	running
		.send_and_wait(channel_id, "done typing", "done typing")
		.await;
	running.session().send(Command::Typing { channel_id });
	running
		.until_wire("typing after the send", |recording| {
			(recording.requests_to("POST", &typing).count() == 2).then_some(())
		})
		.await;
	running.session().send(Command::Typing { channel_id });
	tokio::time::sleep(QUIET).await;
	assert_eq!(running.mock().recording().requests_to("POST", &typing).count(), 2);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slash_command_runs_through_the_interactions_endpoint() {
	let (running, channel_id) = start("composer-slash").await;
	running
		.session()
		.send(Command::LoadApplicationCommands { channel_id });
	running
		.until("the command index to load", |state| {
			state
				.command_index(channel_id)
				.is_some_and(|index| index.command(ECHO_ID).is_some())
		})
		.await;
	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: ECHO_ID,
		options: vec![InteractionOption {
			kind: CommandOptionType::String,
			name: "text".to_owned(),
			value: Some(json!("hello bot")),
			options: Vec::new(),
		}],
	});
	running
		.until("the bot to answer", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache
					.ordered
					.iter()
					.any(|message| message.content == "hello bot" && message.author.id.0 == MOCKBOT_ID)
			})
		})
		.await;
	let recording = running.mock().recording();
	let sent: Vec<&RestRequest> = recording.requests_to("POST", "/interactions").collect();
	assert_eq!(sent.len(), 1);
	let body = String::from_utf8_lossy(&sent[0].body).into_owned();
	let start = body.find('{').expect("a payload_json part");
	let end = body.rfind('}').expect("a payload_json part");
	let payload: Value = serde_json::from_str(&body[start..=end]).expect("payload_json");
	let session_id = running.session().state().read().session().session_id.clone();
	assert!(!session_id.is_empty());
	assert_eq!(payload["type"], json!(2));
	assert_eq!(payload["session_id"], json!(session_id));
	assert_eq!(payload["channel_id"], json!(channel_id.to_string()));
	assert_eq!(payload["application_id"], json!(MOCKBOT_ID.to_string()));
	assert!(payload["nonce"].as_str().is_some_and(|nonce| !nonce.is_empty()));
	assert_eq!(payload["data"]["name"], json!("echo"));
	assert_eq!(payload["data"]["options"][0]["value"], json!("hello bot"));
	assert_eq!(
		payload["data"]["application_command"]["id"],
		json!(ECHO_ID.to_string())
	);
	assert!(payload["guild_id"].is_string());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_command_the_application_fails_is_reported_once() {
	let (running, channel_id) = start("composer-slash-failure").await;
	running
		.session()
		.send(Command::LoadApplicationCommands { channel_id });
	running
		.until("the command index to load", |state| {
			state.command_index(channel_id).is_some()
		})
		.await;
	let before = rejections(&running);
	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: BROKEN_ID,
		options: Vec::new(),
	});
	let reason = rejected_with(&running, before).await;
	assert_eq!(reason, "/broken failed: the application did not respond");
	running
		.mock()
		.respond_once("POST", "/interactions", Reply::captcha_required());
	let before = rejections(&running);
	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: BROKEN_ID,
		options: Vec::new(),
	});
	let reason = rejected_with(&running, before).await;
	assert_eq!(reason, "/broken: complete this in the official Discord client");
	tokio::time::sleep(QUIET).await;
	assert_eq!(
		running
			.mock()
			.recording()
			.requests_to("POST", "/interactions")
			.count(),
		2
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_failed_send_shows_its_reason_and_is_never_retried() {
	let (running, channel_id) = start("composer-failures").await;
	let path = format!("/channels/{channel_id}/messages");
	let cases: [Case; 5] = [
		("refused", Reply::forbidden(), |failure| {
			matches!(failure, SendFailure::Forbidden)
		}),
		("no permission", Reply::missing_permissions(), |failure| {
			matches!(failure, SendFailure::MissingPermissions)
		}),
		("captcha", Reply::captcha_required(), |failure| {
			matches!(failure, SendFailure::Captcha { .. })
		}),
		(
			"server error",
			Reply::error(500, 0, "Internal Server Error"),
			|failure| matches!(failure, SendFailure::Other(Some(500))),
		),
		("slow down", Reply::rate_limited(120.0, false), |failure| {
			matches!(failure, SendFailure::RateLimited { .. })
		}),
	];
	for (content, reply, expected) in cases {
		running.mock().respond_once("POST", &path, reply);
		send(&running, channel_id, content, None, true);
		running
			.until("the send to fail", |state| {
				pending_failure(state, channel_id, content).is_some()
			})
			.await;
		let (_, failure) =
			pending_failure(&running.session().state().read(), channel_id, content).expect("a failure");
		assert!(expected(&failure), "{content}: unexpected {failure:?}");
	}
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	for content in ["refused", "no permission", "captcha", "server error", "slow down"] {
		assert_eq!(posts_of(&recording, content), 1, "{content} was retried");
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_send_is_retried_or_discarded_only_when_asked() {
	let (running, channel_id) = start("composer-retry").await;
	let path = format!("/channels/{channel_id}/messages");
	let target = newest_from_others(&running.session().state().read(), channel_id);
	running.mock().respond_once("POST", &path, Reply::forbidden());
	send(&running, channel_id, "try again", Some(target), false);
	running.mock().respond_once("POST", &path, Reply::forbidden());
	send(&running, channel_id, "give up", None, true);
	running
		.until("both sends to fail", |state| {
			pending_failure(state, channel_id, "try again").is_some()
				&& pending_failure(state, channel_id, "give up").is_some()
		})
		.await;
	let (retry, _) =
		pending_failure(&running.session().state().read(), channel_id, "try again").expect("failed");
	let (discard, _) =
		pending_failure(&running.session().state().read(), channel_id, "give up").expect("failed");
	running.session().send(Command::RetryMessage {
		channel_id,
		nonce: retry,
	});
	running.session().send(Command::DiscardMessage {
		channel_id,
		nonce: discard,
	});
	delivered(&running, channel_id, "try again").await;
	running
		.until("the discarded send to go", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.pending.is_empty())
		})
		.await;
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	assert_eq!(posts_of(&recording, "try again"), 2);
	assert_eq!(posts_of(&recording, "give up"), 1);
	let retried: Vec<Value> = recording
		.requests_to("POST", MESSAGES)
		.filter_map(RestRequest::json)
		.filter(|body| body["content"].as_str() == Some("try again"))
		.collect();
	assert_eq!(retried[1]["allowed_mentions"]["replied_user"], json!(false));
	assert_eq!(
		retried[1]["message_reference"]["message_id"],
		json!(target.to_string())
	);
	assert_ne!(retried[0]["nonce"], retried[1]["nonce"]);
	running.stop().await;
}
