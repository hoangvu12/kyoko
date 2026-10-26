use std::time::Duration;

use mockcord::components::{self as mock, BUG_FILED, PRESSED};
use mockcord::{BUG_COMMAND, InteractionTiming, MOCKBOT_ID, Recording, RestRequest};
use protocol::{ComponentType, Snowflake};
use serde_json::{Value, json};
use session::{Command, ModalAnswer, ModalValue, RestTuning};
use state::{ModalStatus, State};

use crate::harness::{Options, Running};

const QUIET: Duration = Duration::from_millis(600);
const BUG_TEXT: &str = "Reaction tooltip wraps to three lines";

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

async fn posted(
	running: &Running,
	channel_id: Snowflake,
	content: &str,
	components: Vec<Value>,
) -> Snowflake {
	let id = running
		.mock()
		.post_components(channel_id.get(), content, components)
		.map(Snowflake)
		.expect("mockcord posts the component message");
	running
		.until("the component message", |state| {
			has_message(state, channel_id, id)
		})
		.await;
	id
}

fn has_message(state: &State, channel_id: Snowflake, id: Snowflake) -> bool {
	state
		.messages(channel_id)
		.is_some_and(|cache| cache.by_id.contains_key(&id))
}

fn has_content(state: &State, channel_id: Snowflake, content: &str) -> bool {
	state
		.messages(channel_id)
		.is_some_and(|cache| cache.ordered.iter().any(|message| message.content == content))
}

fn press(running: &Running, channel_id: Snowflake, message_id: Snowflake, custom_id: &str) {
	running.session().send(Command::PressComponent {
		channel_id,
		message_id,
		custom_id: custom_id.to_owned(),
		kind: ComponentType::Button,
		values: None,
	});
}

fn interactions(recording: &Recording, kind: i64) -> Vec<(RestRequest, Value)> {
	recording
		.requests_to("POST", "/interactions")
		.filter(|request| !request.injected)
		.filter_map(|request| {
			let body: Value = serde_json::from_slice(&request.body).ok()?;
			Some((request.clone(), body))
		})
		.filter(|(_, body)| body["type"] == json!(kind))
		.collect()
}

fn waiting(state: &State, channel_id: Snowflake, id: Snowflake) -> Option<String> {
	state
		.messages(channel_id)?
		.waiting_component(id)
		.map(str::to_owned)
}

fn failed(state: &State, channel_id: Snowflake, id: Snowflake) -> bool {
	state
		.messages(channel_id)
		.is_some_and(|cache| cache.has_failed_component(id))
}

fn session_id(running: &Running) -> String {
	running.session().state().read().session().session_id.clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_button_press_sends_the_web_clients_component_interaction() {
	let (running, channel_id) = start("components-button").await;
	running.mock().set_interaction_timing(InteractionTiming {
		followup: Duration::from_secs(1),
		..InteractionTiming::default()
	});
	let message_id = posted(&running, channel_id, mock::BUTTONS_PROMPT, mock::buttons()).await;
	press(&running, channel_id, message_id, "refresh");
	running
		.until("the pressed button to wait", |state| {
			waiting(state, channel_id, message_id).as_deref() == Some("refresh")
		})
		.await;
	press(&running, channel_id, message_id, "primary");
	running
		.until("the button to stop waiting", |state| {
			waiting(state, channel_id, message_id).is_none()
		})
		.await;
	assert!(!failed(&running.session().state().read(), channel_id, message_id));
	press(&running, channel_id, message_id, "primary");
	let reply = format!("{PRESSED} Primary");
	running
		.until("Mockbot's answer", |state| has_content(state, channel_id, &reply))
		.await;
	let sent = interactions(&running.mock().recording(), 3);
	assert_eq!(sent.len(), 2, "a press while one is waiting is not sent");
	let (request, body) = &sent[1];
	assert!(
		request
			.header("content-type")
			.is_some_and(|kind| kind.starts_with("application/json")),
		"component interactions are JSON, as the web client posts them"
	);
	let guild_id = running.session().state().read().channel_guild(channel_id);
	assert_eq!(
		*body,
		json!({
			"type": 3,
			"nonce": body["nonce"],
			"guild_id": guild_id.map(|id| id.to_string()),
			"channel_id": channel_id.to_string(),
			"message_flags": 0,
			"message_id": message_id.to_string(),
			"application_id": MOCKBOT_ID.to_string(),
			"session_id": session_id(&running),
			"data": { "component_type": 2, "custom_id": "primary" },
		})
	);
	assert!(body["nonce"].as_str().is_some_and(|nonce| !nonce.is_empty()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choosing_from_a_select_sends_the_values() {
	let (running, channel_id) = start("components-select").await;
	let message_id = posted(
		&running,
		channel_id,
		mock::FRUITS_PROMPT,
		mock::fruit_select("fruits", true),
	)
	.await;
	running.session().send(Command::PressComponent {
		channel_id,
		message_id,
		custom_id: "fruits".to_owned(),
		kind: ComponentType::StringSelect,
		values: Some(vec!["apple".to_owned(), "grapes".to_owned()]),
	});
	let body = running
		.until_wire("the select interaction", |recording| {
			interactions(recording, 3).pop().map(|(_, body)| body)
		})
		.await;
	assert_eq!(
		body["data"],
		json!({
			"component_type": 3,
			"custom_id": "fruits",
			"type": 3,
			"values": ["apple", "grapes"],
		})
	);
	running
		.until("the select to settle", |state| {
			waiting(state, channel_id, message_id).is_none()
		})
		.await;
	assert!(!failed(&running.session().state().read(), channel_id, message_id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_component_interaction_says_this_interaction_failed() {
	let (running, channel_id) = start("components-failure").await;
	running.mock().set_interaction_timing(InteractionTiming {
		window: Duration::from_millis(200),
		..InteractionTiming::default()
	});
	let message_id = posted(&running, channel_id, mock::BUTTONS_PROMPT, mock::buttons()).await;
	press(&running, channel_id, message_id, "danger");
	running
		.until("the failed interaction", |state| {
			failed(state, channel_id, message_id)
		})
		.await;
	assert!(waiting(&running.session().state().read(), channel_id, message_id).is_none());
	press(&running, channel_id, message_id, "success");
	running
		.until("a new press to clear the failure", |state| {
			!failed(state, channel_id, message_id)
		})
		.await;
	let reply = format!("{PRESSED} Success");
	running
		.until("Mockbot's answer", |state| has_content(state, channel_id, &reply))
		.await;
	running.mock().respond_once(
		"POST",
		"/interactions",
		mockcord::Reply::error(400, 50035, "Invalid Form Body"),
	);
	press(&running, channel_id, message_id, "primary");
	running
		.until("the refused interaction", |state| {
			failed(state, channel_id, message_id)
		})
		.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_component_type_never_fails_the_message() {
	let (running, channel_id) = start("components-unknown").await;
	let mut components = mock::buttons();
	components.insert(
		0,
		json!({ "type": 99, "custom_id": "from-the-future", "shiny": true }),
	);
	components.push(json!({ "type": 1, "components": [{ "type": 2, "style": "loud", "label": 7 }] }));
	let message_id = posted(&running, channel_id, mock::BUTTONS_PROMPT, components).await;
	let state = running.session().state().read();
	let message = &state.messages(channel_id).expect("the channel").by_id[&message_id];
	let kinds: Vec<ComponentType> = message
		.components
		.iter()
		.map(|component| component.kind)
		.collect();
	assert_eq!(
		kinds,
		vec![
			ComponentType::Unknown(99),
			ComponentType::ActionRow,
			ComponentType::ActionRow,
			ComponentType::ActionRow,
			ComponentType::ActionRow,
		]
	);
	assert!(message.components[4].components.is_empty());
	assert_eq!(message.components[4].components.dropped(), 1);
	assert_eq!(message.content, mock::BUTTONS_PROMPT);
	assert_eq!(protocol::Component::unknown_kinds(&message.components), vec![99]);
}

async fn open_bug_modal(running: &Running, channel_id: Snowflake) {
	running
		.session()
		.send(Command::LoadApplicationCommands { channel_id });
	running
		.until("the command index", |state| {
			state.command_index(channel_id).is_some()
		})
		.await;
	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: Snowflake(BUG_COMMAND),
		options: Vec::new(),
		files: Vec::new(),
	});
	running
		.until("the app modal", |state| {
			state
				.app_modal()
				.is_some_and(|open| open.modal.title == mock::BUG_TITLE)
		})
		.await;
	running
		.until("the pending row to go", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.pending.is_empty())
		})
		.await;
}

fn answer(custom_id: &str, value: ModalValue) -> ModalAnswer {
	ModalAnswer {
		custom_id: custom_id.to_owned(),
		value,
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_modal_opens_refuses_a_missing_required_field_and_submits() {
	let (running, channel_id) = start("components-modal").await;
	open_bug_modal(&running, channel_id).await;
	running.session().send(Command::SubmitModal {
		answers: vec![answer("title", ModalValue::Text(BUG_TEXT.to_owned()))],
	});
	running
		.until("the missing field", |state| {
			state
				.app_modal()
				.is_some_and(|open| open.missing == ["details"] && open.status == ModalStatus::Open)
		})
		.await;
	tokio::time::sleep(QUIET).await;
	assert!(interactions(&running.mock().recording(), 5).is_empty());
	running.session().send(Command::SubmitModal {
		answers: vec![
			answer("title", ModalValue::Text(BUG_TEXT.to_owned())),
			answer("details", ModalValue::Text("Hover a long reaction".to_owned())),
			answer("severity", ModalValue::Values(vec!["high".to_owned()])),
		],
	});
	let filed = format!("{BUG_FILED} {BUG_TEXT}");
	running
		.until("the modal to close and the answer to arrive", |state| {
			state.app_modal().is_none() && has_content(state, channel_id, &filed)
		})
		.await;
	let (_, body) = interactions(&running.mock().recording(), 5)
		.pop()
		.expect("the modal submit");
	let guild_id = running.session().state().read().channel_guild(channel_id);
	assert_eq!(body["application_id"], json!(MOCKBOT_ID.to_string()));
	assert_eq!(body["channel_id"], json!(channel_id.to_string()));
	assert_eq!(body["guild_id"], json!(guild_id.map(|id| id.to_string())));
	assert_eq!(body["session_id"], json!(session_id(&running)));
	assert_eq!(body["data"]["custom_id"], json!(mock::BUG_MODAL));
	assert!(body["data"]["id"].as_str().is_some());
	assert_eq!(
		body["data"]["components"],
		json!([
			{ "type": 18, "component": { "type": 4, "custom_id": "title", "value": BUG_TEXT } },
			{ "type": 18, "component": { "type": 4, "custom_id": "details", "value": "Hover a long reaction" } },
			{ "type": 18, "component": { "type": 3, "custom_id": "severity", "values": ["high"] } },
		])
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_modal_submit_stays_open_with_the_error_and_cancel_closes_it() {
	let (running, channel_id) = start("components-modal-failed").await;
	running.mock().set_failing_modals(true);
	open_bug_modal(&running, channel_id).await;
	running.session().send(Command::SubmitModal {
		answers: vec![
			answer("title", ModalValue::Text(BUG_TEXT.to_owned())),
			answer("details", ModalValue::Text("Hover a long reaction".to_owned())),
		],
	});
	running
		.until("the modal error", |state| {
			state
				.app_modal()
				.is_some_and(|open| open.status == ModalStatus::Failed)
		})
		.await;
	running.session().send(Command::CloseModal);
	running
		.until("the modal to close", |state| state.app_modal().is_none())
		.await;
}
