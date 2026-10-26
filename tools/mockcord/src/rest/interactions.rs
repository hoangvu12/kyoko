use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::to_bytes;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use chrono::{DateTime, Utc};
use protocol::{CommandUse, MessageType, Snowflake, User};
use serde::Deserialize;
use serde_json::{Value, json};

use super::channels::read_body;
use crate::error::{ApiError, ApiResult};
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
pub const SHRUG_COMMAND: &str = "-1";
const COMMAND_IDS: [(u64, u64, &str); 7] = [
	(9_100_000_000_000_000_101, MOCKBOT_ID, "ping"),
	(9_100_000_000_000_000_102, MOCKBOT_ID, "echo"),
	(ROLL_COMMAND, MOCKBOT_ID, "roll"),
	(9_100_000_000_000_000_104, MOCKBOT_ID, "broken"),
	(REMIND_COMMAND, REMINDY_ID, "remind"),
	(9_100_000_000_000_000_301, POLLY_ID, "poll"),
	(9_100_000_000_000_000_401, SNAPPER_ID, "snap"),
];
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
					"description": "The language to answer in",
					"required": true,
					"choices": [
						{ "name": "English", "value": "en" },
						{ "name": "French", "value": "fr" },
						{ "name": "Japanese", "value": "ja" },
					],
				},
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

fn answer(name: &str, data: &Value) -> Option<String> {
	match name {
		"ping" => Some("Pong!".to_owned()),
		"echo" => option_value(data, "text")
			.and_then(Value::as_str)
			.map(str::to_owned),
		"roll" => {
			let sides = option_value(data, "sides").and_then(Value::as_i64).unwrap_or(6);
			Some(format!("You rolled {} of {sides}", sides.min(4)))
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
	if body["type"].as_i64() != Some(2) || body["session_id"].as_str().is_none_or(str::is_empty) {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	}
	let nonce = body["nonce"].clone();
	let interaction_id = shared.store.lock().next_snowflake().to_string();
	let event = json!({ "id": interaction_id, "nonce": nonce });
	shared.hub.broadcast("INTERACTION_CREATE", &event);
	let Some(content) = answer(name, data) else {
		shared.hub.broadcast("INTERACTION_FAILURE", &event);
		return Ok(StatusCode::NO_CONTENT);
	};
	shared.hub.broadcast("INTERACTION_SUCCESS", &event);
	let draft = Draft {
		kind: Some(MessageType::ChatInputCommand),
		..Draft::text(content)
	};
	shared.post_message(channel_id, &mockbot(), draft)?;
	Ok(StatusCode::NO_CONTENT)
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
