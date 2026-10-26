use std::time::Duration;

use mockcord::{
	AutocompleteReply, MOCKBOT_ID, RESIZE_COMMAND, Recording, Reply, RestRequest, TRANSLATE_COMMAND,
};
use protocol::{CommandOptionType, InteractionOption, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning, UploadFile};
use state::{AutocompleteStatus, State};

use crate::harness::{Options, Running};

const ECHO_ID: Snowflake = Snowflake(9_100_000_000_000_000_102);
const TRANSLATE_ID: Snowflake = Snowflake(TRANSLATE_COMMAND);
const RESIZE_ID: Snowflake = Snowflake(RESIZE_COMMAND);
const QUIET: Duration = Duration::from_millis(600);
const SEND_GAP: Duration = Duration::from_millis(1200);
const GAP_SLACK: Duration = Duration::from_millis(50);
const AUTOCOMPLETE: i64 = 4;
const RUN: i64 = 2;

fn unpaced() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

async fn started(name: &'static str, tuning: RestTuning) -> (Running, Snowflake) {
	let running = Running::start_with(Options::new(name).rest(tuning)).await;
	let channel_id = running.open_text_channel().await;
	running
		.session()
		.send(Command::LoadApplicationCommands { channel_id });
	running
		.until("the command index to load", |state| {
			state
				.command_index(channel_id)
				.is_some_and(|index| index.command(TRANSLATE_ID).is_some())
		})
		.await;
	(running, channel_id)
}

fn option(kind: CommandOptionType, name: &str, value: Value, focused: Option<bool>) -> InteractionOption {
	InteractionOption {
		kind,
		name: name.to_owned(),
		value: Some(value),
		options: Vec::new(),
		focused,
	}
}

fn ask(running: &Running, channel_id: Snowflake, typed: &str, request: u64) {
	running.session().send(Command::AutocompleteCommand {
		channel_id,
		command_id: TRANSLATE_ID,
		options: vec![
			option(CommandOptionType::String, "text", json!("hi"), None),
			option(CommandOptionType::String, "language", json!(typed), Some(true)),
		],
		request,
	});
}

fn status_of(state: &State, request: u64) -> Option<AutocompleteStatus> {
	state
		.autocomplete()
		.filter(|autocomplete| autocomplete.request == request)
		.map(|autocomplete| autocomplete.status.clone())
}

async fn settled(running: &Running, request: u64) -> AutocompleteStatus {
	running
		.until("the autocomplete to settle", |state| {
			status_of(state, request).is_some_and(|status| status != AutocompleteStatus::Loading)
		})
		.await;
	status_of(&running.session().state().read(), request).expect("a settled autocomplete")
}

fn names(status: &AutocompleteStatus) -> Vec<String> {
	match status {
		AutocompleteStatus::Loaded(choices) => choices.iter().map(|choice| choice.name.clone()).collect(),
		other => panic!("expected choices, got {other:?}"),
	}
}

fn payload(request: &RestRequest) -> Value {
	let body = String::from_utf8_lossy(&request.body).into_owned();
	let start = body.find('{').expect("a payload_json part");
	let end = body.rfind('}').expect("a payload_json part");
	serde_json::from_str(&body[start..=end]).expect("payload_json")
}

fn interactions(recording: &Recording, kind: i64) -> Vec<(RestRequest, Value)> {
	recording
		.requests_to("POST", "/interactions")
		.filter(|request| !request.injected)
		.map(|request| (request.clone(), payload(request)))
		.filter(|(_, body)| body["type"] == json!(kind))
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn autocomplete_sends_the_focused_value_and_brings_back_the_choices() {
	let (running, channel_id) = started("command-autocomplete", unpaced()).await;
	ask(&running, channel_id, "deu", 1);
	running
		.until("the autocomplete to load", |state| {
			status_of(state, 1) == Some(AutocompleteStatus::Loading)
		})
		.await;
	let status = settled(&running, 1).await;
	assert_eq!(names(&status), ["Deutsch"]);
	let sent = interactions(&running.mock().recording(), AUTOCOMPLETE);
	assert_eq!(sent.len(), 1);
	let body = &sent[0].1;
	let session_id = running.session().state().read().session().session_id.clone();
	assert_eq!(body["session_id"], json!(session_id));
	assert_eq!(body["channel_id"], json!(channel_id.to_string()));
	assert_eq!(body["application_id"], json!(MOCKBOT_ID.to_string()));
	assert!(body["nonce"].as_str().is_some_and(|nonce| !nonce.is_empty()));
	assert!(body.get("analytics_location").is_none());
	assert_eq!(body["data"]["name"], json!("translate"));
	assert_eq!(
		body["data"]["options"][0],
		json!({ "type": 3, "name": "text", "value": "hi" })
	);
	assert_eq!(
		body["data"]["options"][1],
		json!({ "type": 3, "name": "language", "value": "deu", "focused": true })
	);
	assert!(
		running
			.session()
			.state()
			.read()
			.messages(channel_id)
			.is_some_and(|cache| cache
				.ordered
				.iter()
				.all(|message| message.author.id.0 != MOCKBOT_ID)),
		"an autocomplete posts nothing"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typing_is_debounced_into_one_autocomplete_paced_like_other_interactions() {
	let tuning = RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: SEND_GAP,
		..RestTuning::default()
	};
	let (running, channel_id) = started("command-autocomplete-pacing", tuning).await;
	for (request, typed) in ["e", "es", "esp", "espa", "españ"].into_iter().enumerate() {
		ask(&running, channel_id, typed, request as u64 + 1);
	}
	let status = settled(&running, 5).await;
	assert_eq!(names(&status), ["Español"]);
	tokio::time::sleep(QUIET).await;
	let sent = interactions(&running.mock().recording(), AUTOCOMPLETE);
	assert_eq!(sent.len(), 1, "keystrokes inside the debounce send one request");
	assert_eq!(sent[0].1["data"]["options"][1]["value"], json!("españ"));

	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: ECHO_ID,
		options: vec![
			option(CommandOptionType::String, "text", json!("paced"), None),
			option(CommandOptionType::String, "language", json!("en"), None),
		],
		files: Vec::new(),
	});
	ask(&running, channel_id, "fr", 6);
	assert_eq!(names(&settled(&running, 6).await), ["Français"]);
	let recording = running.mock().recording();
	let run = interactions(&recording, RUN);
	let asked = interactions(&recording, AUTOCOMPLETE);
	assert_eq!((run.len(), asked.len()), (1, 2));
	let gap = asked[1].0.at.duration_since(run[0].0.at);
	assert!(
		gap + GAP_SLACK >= SEND_GAP,
		"the autocomplete waited {gap:?} after the command"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_autocomplete_says_so_and_a_retry_loads_the_choices() {
	let (running, channel_id) = started("command-autocomplete-failure", unpaced()).await;
	running.mock().respond_once(
		"POST",
		"/interactions",
		Reply::error(500, 0, "Internal Server Error"),
	);
	ask(&running, channel_id, "ja", 1);
	assert_eq!(settled(&running, 1).await, AutocompleteStatus::Failed);
	ask(&running, channel_id, "ja", 2);
	assert_eq!(names(&settled(&running, 2).await), ["日本語"]);

	running.mock().set_autocomplete(AutocompleteReply::Fail);
	ask(&running, channel_id, "ja", 3);
	assert_eq!(settled(&running, 3).await, AutocompleteStatus::Failed);

	running.mock().set_autocomplete(AutocompleteReply::Stall);
	ask(&running, channel_id, "ja", 4);
	tokio::time::sleep(QUIET * 2).await;
	assert_eq!(
		status_of(&running.session().state().read(), 4),
		Some(AutocompleteStatus::Loading),
		"an app that has not answered yet is still loading"
	);
	assert_eq!(
		settled(&running, 4).await,
		AutocompleteStatus::Failed,
		"an app that never answers times out"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_file_option_uploads_to_a_slot_then_runs_with_the_attachment() {
	let (running, channel_id) = started("command-file-option", unpaced()).await;
	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: RESIZE_ID,
		options: vec![
			option(CommandOptionType::Attachment, "file", json!(0), None),
			option(CommandOptionType::Integer, "width", json!(200), None),
		],
		files: vec![UploadFile::new("cat.png", vec![7u8; 2048], "image/png")],
	});
	running
		.until("the bot to answer", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.ordered.iter().any(|message| {
					message.author.id.0 == MOCKBOT_ID
						&& message.content == "Resized cat.png to 200 pixels wide"
				})
			})
		})
		.await;
	let recording = running.mock().recording();
	let slot_path = format!("/channels/{channel_id}/attachments");
	let slots: Vec<&RestRequest> = recording.requests_to("POST", &slot_path).collect();
	assert_eq!(slots.len(), 1);
	let slot = slots[0].json().expect("a json slot request");
	assert_eq!(slot["files"][0]["filename"], json!("cat.png"));
	assert_eq!(slot["files"][0]["file_size"], json!(2048));
	let run = interactions(&recording, RUN);
	assert_eq!(run.len(), 1);
	assert!(
		slots[0].at <= run[0].0.at,
		"the file is uploaded before the command runs"
	);
	let data = &run[0].1["data"];
	assert_eq!(
		data["options"][0],
		json!({ "type": 11, "name": "file", "value": 0 })
	);
	assert_eq!(data["attachments"][0]["id"], json!("0"));
	assert_eq!(data["attachments"][0]["filename"], json!("cat.png"));
	assert!(
		data["attachments"][0]["uploaded_filename"]
			.as_str()
			.is_some_and(|name| name.ends_with("/cat.png"))
	);
	running.stop().await;
}
