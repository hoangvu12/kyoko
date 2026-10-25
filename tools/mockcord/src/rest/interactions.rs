use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use protocol::{MessageType, Snowflake, User};
use serde::Deserialize;
use serde_json::{Value, json};

use super::channels::read_body;
use crate::error::{ApiError, ApiResult};
use crate::ops::{Draft, UploadedFile};
use crate::state::Shared;

pub const MOCKBOT_ID: u64 = 9_100_000_000_000_000_001;
const COMMAND_IDS: [(u64, &str); 4] = [
	(9_100_000_000_000_000_101, "ping"),
	(9_100_000_000_000_000_102, "echo"),
	(9_100_000_000_000_000_103, "roll"),
	(9_100_000_000_000_000_104, "broken"),
];
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

fn command(id: u64, name: &str) -> Value {
	let (description, options) = match name {
		"ping" => ("Check that the bot answers", json!([])),
		"echo" => (
			"Repeat some text",
			json!([{ "type": 3, "name": "text", "description": "What to repeat", "required": true }]),
		),
		"roll" => (
			"Roll a die",
			json!([{
				"type": 4,
				"name": "sides",
				"description": "How many sides",
				"required": false,
				"choices": [{ "name": "six", "value": 6 }, { "name": "twenty", "value": 20 }],
			}]),
		),
		_ => ("Never answers", json!([])),
	};
	json!({
		"id": id.to_string(),
		"type": 1,
		"application_id": MOCKBOT_ID.to_string(),
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
		"applications": [{
			"id": MOCKBOT_ID.to_string(),
			"name": "Mockbot",
			"icon": null,
			"bot_id": MOCKBOT_ID.to_string(),
		}],
		"application_commands": COMMAND_IDS.iter().map(|(id, name)| command(*id, name)).collect::<Vec<_>>(),
		"version": COMMAND_VERSION.to_string(),
	})
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

pub async fn upload(
	State(shared): State<Arc<Shared>>,
	Path(key): Path<String>,
	headers: HeaderMap,
	body: Bytes,
) -> StatusCode {
	let filename = key.rsplit('/').next().unwrap_or("file").to_owned();
	let content_type = headers
		.get(CONTENT_TYPE)
		.and_then(|value| value.to_str().ok())
		.map(str::to_owned);
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
		.and_then(|id| COMMAND_IDS.iter().find(|(known, _)| *known == id));
	let channel_id = body["channel_id"].as_str().and_then(|id| id.parse::<u64>().ok());
	let (Some((_, name)), Some(channel_id)) = (known, channel_id) else {
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
