use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::to_bytes;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use chrono::{DateTime, Utc};
use protocol::{CommandUse, MessageFlags, MessageType, Snowflake, User};
use serde::Deserialize;
use serde_json::{Value, json};

use super::channels::read_body;
use crate::config::AutocompleteReply;
use crate::error::{ApiError, ApiResult};
use crate::fixtures::CURRENT_USER_ID;
use crate::json::to_json;
use crate::ops::{Draft, UploadedFile};
use crate::state::Shared;

pub const MOCKBOT_ID: u64 = 9_100_000_000_000_000_001;
pub const STALLED_UPLOAD: &str = "stalled-upload.mp4";
pub const BROKEN_UPLOAD: &str = "broken-upload.mp4";
const STALL: Duration = Duration::from_secs(600);
const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;
pub const REMINDY_ID: u64 = 9_100_000_000_000_000_002;
pub const POLLY_ID: u64 = 9_100_000_000_000_000_003;
pub const SNAPPER_ID: u64 = 9_100_000_000_000_000_004;
pub const REMIND_COMMAND: u64 = 9_100_000_000_000_000_201;
pub const ROLL_COMMAND: u64 = 9_100_000_000_000_000_103;
pub const TRANSLATE_COMMAND: u64 = 9_100_000_000_000_000_105;
pub const RESIZE_COMMAND: u64 = 9_100_000_000_000_000_402;
pub const PING_COMMAND: u64 = 9_100_000_000_000_000_101;
pub const BROKEN_COMMAND: u64 = 9_100_000_000_000_000_104;
pub const THINK_COMMAND: u64 = 9_100_000_000_000_000_106;
pub const WHISPER_COMMAND: u64 = 9_100_000_000_000_000_107;
pub const IGNORE_COMMAND: u64 = 9_100_000_000_000_000_108;
pub const THOUGHT: &str = "Thought it over: the answer is 42";
pub const WHISPERED: &str = "Only you can read this one";
pub const SHRUG_COMMAND: &str = "-1";
pub const LANGUAGES: [(&str, &str); 6] = [
	("English", "en"),
	("Deutsch", "de"),
	("Español", "es"),
	("Français", "fr"),
	("日本語", "ja"),
	("Português", "pt"),
];
const COMMAND_IDS: [(u64, u64, &str); 12] = [
	(PING_COMMAND, MOCKBOT_ID, "ping"),
	(9_100_000_000_000_000_102, MOCKBOT_ID, "echo"),
	(ROLL_COMMAND, MOCKBOT_ID, "roll"),
	(BROKEN_COMMAND, MOCKBOT_ID, "broken"),
	(TRANSLATE_COMMAND, MOCKBOT_ID, "translate"),
	(THINK_COMMAND, MOCKBOT_ID, "think"),
	(WHISPER_COMMAND, MOCKBOT_ID, "whisper"),
	(IGNORE_COMMAND, MOCKBOT_ID, "ignore"),
	(REMIND_COMMAND, REMINDY_ID, "remind"),
	(9_100_000_000_000_000_301, POLLY_ID, "poll"),
	(9_100_000_000_000_000_401, SNAPPER_ID, "snap"),
	(RESIZE_COMMAND, SNAPPER_ID, "resize"),
];
const APPLICATION_COMMAND: i64 = 2;
const APPLICATION_COMMAND_AUTOCOMPLETE: i64 = 4;
const APPLICATIONS: [(u64, &str); 4] = [
	(MOCKBOT_ID, "Mockbot"),
	(REMINDY_ID, "Remindy"),
	(POLLY_ID, "Polly"),
	(SNAPPER_ID, "Snapper"),
];
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const COMMAND_VERSION: u64 = 9_100_000_000_000_000_900;

#[derive(Deserialize, Default)]
#[serde(default)]
struct SlotRequest {
	#[serde(deserialize_with = "protocol::lenient::int")]
	id: i64,
	filename: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SlotsRequest {
	files: Vec<SlotRequest>,
}

fn mockbot() -> User {
	User {
		id: Snowflake(MOCKBOT_ID),
		username: "mockbot".to_owned(),
		global_name: Some("Mockbot".to_owned()),
		bot: Some(true),
		..User::default()
	}
}

fn string_option(name: &str, description: &str, required: bool) -> Value {
	json!({ "type": 3, "name": name, "description": description, "required": required })
}

fn integer_option(name: &str, description: &str, required: bool, max: i64) -> Value {
	json!({
		"type": 4,
		"name": name,
		"description": description,
		"required": required,
		"min_value": 1,
		"max_value": max,
	})
}

fn language_choices(typed: &str) -> Vec<Value> {
	let typed = typed.to_lowercase();
	LANGUAGES
		.iter()
		.filter(|(name, value)| name.to_lowercase().contains(&typed) || value.starts_with(&typed))
		.map(|(name, value)| json!({ "name": name, "value": value }))
		.collect()
}

fn command(id: u64, application_id: u64, name: &str) -> Value {
	let (description, options) = match name {
		"ping" => ("Check that Mockbot is responding", json!([])),
		"echo" => (
			"Repeats what you say",
			json!([
				string_option("text", "What to repeat", true),
				{
					"type": 3,
					"name": "language",
					"description": "The language to echo in",
					"required": true,
					"choices": language_choices(""),
				},
			]),
		),
		"translate" => (
			"Translates what you say",
			json!([
				string_option("text", "What to translate", true),
				{
					"type": 3,
					"name": "language",
					"description": "The language to translate to",
					"required": true,
					"autocomplete": true,
				},
			]),
		),
		"resize" => (
			"Resize an image",
			json!([
				{ "type": 11, "name": "file", "description": "The image to resize", "required": true },
				integer_option("width", "New width in pixels", true, 4096),
			]),
		),
		"roll" => (
			"Roll a die",
			json!([
				integer_option("sides", "How many sides the die has", true, 100),
				integer_option("count", "How many dice to roll", false, 10),
			]),
		),
		"remind" => (
			"Set a reminder",
			json!([
				string_option("when", "When to remind you, like 'in 2 hours'", true),
				string_option("message", "What to remind you about", true),
			]),
		),
		"poll" => (
			"Ask the channel a question",
			json!([string_option("question", "What to ask", true)]),
		),
		"snap" => ("Save a snapshot of this channel", json!([])),
		"think" => ("Takes a moment before answering", json!([])),
		"whisper" => ("Answers so only you can see it", json!([])),
		"ignore" => ("Never answers, to test timeouts", json!([])),
		_ => ("Always fails, to test error states", json!([])),
	};
	json!({
		"id": id.to_string(),
		"type": 1,
		"application_id": application_id.to_string(),
		"version": COMMAND_VERSION.to_string(),
		"name": name,
		"description": description,
		"options": options,
		"integration_types": [0],
		"contexts": [0, 1, 2],
	})
}

fn index() -> Value {
	json!({
		"applications": APPLICATIONS.iter().map(|(id, name)| json!({
			"id": id.to_string(),
			"name": name,
			"icon": null,
			"bot_id": id.to_string(),
		})).collect::<Vec<_>>(),
		"application_commands": COMMAND_IDS
			.iter()
			.map(|(id, application_id, name)| command(*id, *application_id, name))
			.collect::<Vec<_>>(),
		"version": COMMAND_VERSION.to_string(),
	})
}

pub fn frequent_commands(anchor: DateTime<Utc>) -> Vec<CommandUse> {
	let now = anchor.timestamp_millis();
	[
		(REMIND_COMMAND.to_string(), 12),
		(ROLL_COMMAND.to_string(), 7),
		(SHRUG_COMMAND.to_owned(), 3),
	]
	.into_iter()
	.map(|(key, uses)| CommandUse {
		key,
		total_uses: uses,
		recent_uses: (0..uses.min(10))
			.map(|day| (now - i64::from(day) * DAY_MS) as u64)
			.collect(),
		frecency: -1,
		score: -1,
	})
	.collect()
}
pub async fn guild_index(State(shared): State<Arc<Shared>>, Path(id): Path<u64>) -> ApiResult {
	if !shared.fixtures.guilds.iter().any(|guild| guild.id == id) {
		return Err(ApiError::unknown_guild());
	}
	Ok(Json(index()))
}

pub async fn channel_index(State(shared): State<Arc<Shared>>, Path(id): Path<u64>) -> ApiResult {
	if !shared.store.lock().channels.contains_key(&id) {
		return Err(ApiError::unknown_channel());
	}
	Ok(Json(index()))
}

pub async fn user_index() -> ApiResult {
	Ok(Json(
		json!({ "applications": [], "application_commands": [], "version": "0" }),
	))
}

pub async fn attachment_slots(
	State(shared): State<Arc<Shared>>,
	Path(id): Path<u64>,
	Json(body): Json<Value>,
) -> ApiResult {
	let request: SlotsRequest =
		serde_json::from_value(body).map_err(|_| ApiError::invalid_body("Invalid Form Body"))?;
	if request.files.is_empty() {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	}
	let mut store = shared.store.lock();
	if !store.channels.contains_key(&id) {
		return Err(ApiError::unknown_channel());
	}
	let slots: Vec<Value> = request
		.files
		.iter()
		.map(|file| {
			let name: String = file
				.filename
				.chars()
				.map(|character| {
					if character.is_ascii_alphanumeric() || character == '.' {
						character
					} else {
						'_'
					}
				})
				.collect();
			let key = format!("{}/{name}", store.next_snowflake());
			json!({
				"id": file.id,
				"upload_url": format!("{}/upload/{key}", shared.urls.origin),
				"upload_filename": key,
			})
		})
		.collect();
	Ok(Json(json!({ "attachments": slots })))
}

fn slot_name(filename: &str) -> String {
	filename
		.chars()
		.map(|character| {
			if character.is_ascii_alphanumeric() || character == '.' {
				character
			} else {
				'_'
			}
		})
		.collect()
}

pub async fn upload(
	State(shared): State<Arc<Shared>>,
	Path(key): Path<String>,
	request: Request,
) -> StatusCode {
	let filename = key.rsplit('/').next().unwrap_or("file").to_owned();
	if filename == slot_name(STALLED_UPLOAD) {
		tokio::time::sleep(STALL).await;
	}
	if filename == slot_name(BROKEN_UPLOAD) {
		return StatusCode::INTERNAL_SERVER_ERROR;
	}
	let content_type = request
		.headers()
		.get(CONTENT_TYPE)
		.and_then(|value| value.to_str().ok())
		.map(str::to_owned);
	let Ok(body) = to_bytes(request.into_body(), MAX_UPLOAD_BYTES).await else {
		return StatusCode::BAD_REQUEST;
	};
	shared.store.lock().uploads.insert(
		key,
		UploadedFile {
			filename,
			content_type,
			size: body.len(),
		},
	);
	StatusCode::OK
}

fn option_value<'a>(data: &'a Value, name: &str) -> Option<&'a Value> {
	data["options"]
		.as_array()?
		.iter()
		.find(|option| option["name"].as_str() == Some(name))
		.map(|option| &option["value"])
}

fn focused_value(data: &Value) -> Option<&str> {
	data["options"]
		.as_array()?
		.iter()
		.find(|option| option["focused"].as_bool() == Some(true))
		.and_then(|option| option["value"].as_str())
}

fn attachment_id(value: &Value) -> Option<u64> {
	value
		.as_u64()
		.or_else(|| value.as_str().and_then(|id| id.parse().ok()))
}

fn claimed_attachment(shared: &Shared, data: &Value, name: &str) -> Option<UploadedFile> {
	let wanted = attachment_id(option_value(data, name)?)?;
	let attachment = data["attachments"]
		.as_array()?
		.iter()
		.find(|attachment| attachment_id(&attachment["id"]) == Some(wanted))?;
	let key = attachment["uploaded_filename"].as_str()?;
	shared.store.lock().uploads.remove(key)
}

async fn autocomplete(shared: Arc<Shared>, body: &Value, name: &str) -> Result<StatusCode, ApiError> {
	if name != "translate" {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	}
	let reply = shared.store.lock().autocomplete;
	if reply == AutocompleteReply::Fail {
		return Err(ApiError::server_error());
	}
	let typed = focused_value(&body["data"]).unwrap_or_default().to_owned();
	let event = json!({
		"nonce": body["nonce"].clone(),
		"application_id": MOCKBOT_ID.to_string(),
		"choices": language_choices(&typed),
	});
	if reply == AutocompleteReply::Stall {
		tokio::spawn(async move {
			tokio::time::sleep(STALL).await;
			shared
				.hub
				.broadcast("APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE", &event);
		});
	} else {
		shared
			.hub
			.broadcast("APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE", &event);
	}
	Ok(StatusCode::NO_CONTENT)
}

fn answer(name: &str, data: &Value) -> Option<String> {
	match name {
		"ping" => Some("Pong!".to_owned()),
		"echo" => option_value(data, "text")
			.and_then(Value::as_str)
			.map(str::to_owned),
		"roll" => {
			let sides = option_value(data, "sides").and_then(Value::as_i64).unwrap_or(6);
			Some(format!("\u{1f3b2} You rolled a {}", sides.min(17)))
		}
		"remind" => {
			let when = option_value(data, "when").and_then(Value::as_str)?;
			let message = option_value(data, "message").and_then(Value::as_str)?;
			Some(format!("I will remind you {when}: {message}"))
		}
		"poll" => option_value(data, "question")
			.and_then(Value::as_str)
			.map(|question| format!("Poll: {question}")),
		"snap" => Some("Snapshot saved".to_owned()),
		"translate" => {
			let text = option_value(data, "text").and_then(Value::as_str)?;
			let language = option_value(data, "language").and_then(Value::as_str)?;
			Some(format!("{text} ({language})"))
		}
		_ => None,
	}
}

pub async fn create_interaction(
	State(shared): State<Arc<Shared>>,
	request: Request,
) -> Result<StatusCode, ApiError> {
	let (body, _) = read_body(request).await?;
	let data = &body["data"];
	let known = data["id"]
		.as_str()
		.and_then(|id| id.parse::<u64>().ok())
		.and_then(|id| COMMAND_IDS.iter().find(|(known, ..)| *known == id));
	let channel_id = body["channel_id"].as_str().and_then(|id| id.parse::<u64>().ok());
	let (Some((_, _, name)), Some(channel_id)) = (known, channel_id) else {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	};
	if body["session_id"].as_str().is_none_or(str::is_empty) {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	}
	match body["type"].as_i64() {
		Some(APPLICATION_COMMAND) => {}
		Some(APPLICATION_COMMAND_AUTOCOMPLETE) => return autocomplete(shared, &body, name).await,
		_ => return Err(ApiError::invalid_body("Invalid Form Body")),
	}
	let mut files = Vec::new();
	if *name == "resize" {
		let file =
			claimed_attachment(&shared, data, "file").ok_or(ApiError::invalid_body("Invalid upload"))?;
		files.push(file);
	}
	let nonce = body["nonce"].clone();
	let (interaction_id, timing) = {
		let mut store = shared.store.lock();
		(store.next_snowflake().to_string(), store.interactions)
	};
	if *name == "ping" && timing.hold_ping {
		return Ok(StatusCode::NO_CONTENT);
	}
	let event = json!({ "id": interaction_id, "nonce": nonce });
	shared.hub.broadcast("INTERACTION_CREATE", &event);
	let responder = Responder {
		nonce: nonce.as_str().map(str::to_owned),
		interaction: metadata(&shared, &interaction_id, name),
		channel_id,
	};
	match *name {
		"ignore" => {
			tokio::spawn(async move {
				tokio::time::sleep(timing.window).await;
				shared.hub.broadcast("INTERACTION_FAILURE", &event);
			});
			return Ok(StatusCode::NO_CONTENT);
		}
		"think" => {
			shared.hub.broadcast("INTERACTION_SUCCESS", &event);
			let posted = responder.post(&shared, String::new(), MessageFlags::LOADING)?;
			let id = posted["id"].as_str().and_then(|id| id.parse::<u64>().ok());
			tokio::spawn(async move {
				tokio::time::sleep(timing.followup).await;
				if let Some(id) = id {
					let _ = shared.complete_deferred(channel_id, id, THOUGHT);
				}
			});
			return Ok(StatusCode::NO_CONTENT);
		}
		"whisper" => {
			shared.hub.broadcast("INTERACTION_SUCCESS", &event);
			responder.post(&shared, WHISPERED.to_owned(), MessageFlags::EPHEMERAL)?;
			return Ok(StatusCode::NO_CONTENT);
		}
		_ => {}
	}
	let content = match files.first() {
		Some(file) => option_value(data, "width")
			.and_then(Value::as_i64)
			.map(|width| format!("Resized {} to {width} pixels wide", file.filename)),
		None => answer(name, data),
	};
	let Some(content) = content else {
		shared.hub.broadcast("INTERACTION_FAILURE", &event);
		return Ok(StatusCode::NO_CONTENT);
	};
	shared.hub.broadcast("INTERACTION_SUCCESS", &event);
	let draft = Draft {
		files,
		..responder.draft(content, MessageFlags::empty())
	};
	shared.post_message(channel_id, &mockbot(), draft)?;
	Ok(StatusCode::NO_CONTENT)
}

struct Responder {
	nonce: Option<String>,
	interaction: Value,
	channel_id: u64,
}

impl Responder {
	fn draft(&self, content: String, flags: MessageFlags) -> Draft {
		Draft {
			kind: Some(MessageType::ChatInputCommand),
			nonce: self.nonce.clone(),
			flags,
			interaction: self.interaction.clone(),
			..Draft::text(content)
		}
	}

	fn post(&self, shared: &Shared, content: String, flags: MessageFlags) -> Result<Value, ApiError> {
		shared.post_message(self.channel_id, &mockbot(), self.draft(content, flags))
	}
}

fn metadata(shared: &Shared, interaction_id: &str, name: &str) -> Value {
	let user = shared
		.fixtures
		.users
		.get(&CURRENT_USER_ID)
		.map_or(Value::Null, to_json);
	json!({
		"id": interaction_id,
		"type": APPLICATION_COMMAND,
		"name": name,
		"command_type": 1,
		"user": user,
		"authorizing_integration_owners": { "0": "0" },
	})
}

pub fn claim_uploads(shared: &Shared, draft: &mut Draft) -> Result<(), ApiError> {
	let keys: Vec<String> = draft
		.descriptors
		.iter()
		.filter_map(|descriptor| descriptor.uploaded_filename.clone())
		.collect();
	if keys.is_empty() {
		return Ok(());
	}
	let mut store = shared.store.lock();
	for key in keys {
		let file = store
			.uploads
			.remove(&key)
			.ok_or(ApiError::invalid_body("Invalid upload"))?;
		draft.files.push(file);
	}
	Ok(())
}
