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
use crate::components;
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
pub const BUG_COMMAND: u64 = 9_100_000_000_000_000_109;
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
const COMMAND_IDS: [(u64, u64, &str); 13] = [
	(PING_COMMAND, MOCKBOT_ID, "ping"),
	(9_100_000_000_000_000_102, MOCKBOT_ID, "echo"),
	(ROLL_COMMAND, MOCKBOT_ID, "roll"),
	(BROKEN_COMMAND, MOCKBOT_ID, "broken"),
	(TRANSLATE_COMMAND, MOCKBOT_ID, "translate"),
	(THINK_COMMAND, MOCKBOT_ID, "think"),
	(WHISPER_COMMAND, MOCKBOT_ID, "whisper"),
	(IGNORE_COMMAND, MOCKBOT_ID, "ignore"),
	(BUG_COMMAND, MOCKBOT_ID, "bug"),
	(REMIND_COMMAND, REMINDY_ID, "remind"),
	(POLL_COMMAND, POLLY_ID, "poll"),
	(SNAP_COMMAND, SNAPPER_ID, "snap"),
	(RESIZE_COMMAND, SNAPPER_ID, "resize"),
];
const APPLICATION_COMMAND: i64 = 2;
const APPLICATION_COMMAND_AUTOCOMPLETE: i64 = 4;
const MESSAGE_COMPONENT: i64 = 3;
const MODAL_SUBMIT: i64 = 5;
const APPLICATIONS: [(u64, &str, &str); 4] = [
	(
		MOCKBOT_ID,
		"Mockbot",
		"Test bot for kyoko's command and component states",
	),
	(REMINDY_ID, "Remindy", "Reminders for you and your server"),
	(POLLY_ID, "Polly", "Quick polls with buttons and live results"),
	(SNAPPER_ID, "Snapper", "Resize, crop and convert images"),
];
pub const WARDEN_ID: u64 = 9_100_000_000_000_000_005;
pub const LYRA_ID: u64 = 9_100_000_000_000_000_006;
pub const TICKETEER_ID: u64 = 9_100_000_000_000_000_007;
pub const COUNTR_ID: u64 = 9_100_000_000_000_000_008;
pub const ARCADE_ID: u64 = 9_100_000_000_000_000_009;
pub const POLL_COMMAND: u64 = 9_100_000_000_000_000_301;
pub const SNAP_COMMAND: u64 = 9_100_000_000_000_000_401;
pub const WARN_COMMAND: u64 = 9_100_000_000_000_000_501;
pub const PLAY_COMMAND: u64 = 9_100_000_000_000_000_601;
const LAUNCH_COMMAND: u64 = 9_100_000_000_000_000_951;
const MORE_APPLICATIONS: [(u64, &str, &str); 5] = [
	(WARDEN_ID, "Warden", "Moderation, logging and reaction roles"),
	(LYRA_ID, "Lyra", "Music and playlists for voice channels"),
	(TICKETEER_ID, "Ticketeer", "Support tickets in private threads"),
	(COUNTR_ID, "Countr", "A counting game for one channel"),
	(ARCADE_ID, "Arcade", "Party games for voice channels"),
];
const MORE_COMMANDS: [(u64, u64, &str); 4] = [
	(WARN_COMMAND, WARDEN_ID, "warn"),
	(PLAY_COMMAND, LYRA_ID, "play"),
	(9_100_000_000_000_000_701, TICKETEER_ID, "ticket"),
	(9_100_000_000_000_000_801, COUNTR_ID, "count"),
];
const PRIMARY_ENTRY_POINT: i64 = 4;
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

pub(crate) fn mockbot() -> User {
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
		"bug" => ("Report a bug in a form", json!([])),
		"warn" => (
			"Warn a member",
			json!([string_option("reason", "Why they are warned", true)]),
		),
		"play" => (
			"Play a song in your voice channel",
			json!([string_option("song", "What to play", true)]),
		),
		"ticket" => ("Open a support ticket", json!([])),
		"count" => ("Show the current count", json!([])),
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

fn application(id: u64, name: &str, description: &str) -> Value {
	json!({
		"id": id.to_string(),
		"name": name,
		"description": description,
		"icon": null,
		"bot_id": id.to_string(),
	})
}

fn entry_point(id: u64, application_id: u64) -> Value {
	json!({
		"id": id.to_string(),
		"type": PRIMARY_ENTRY_POINT,
		"application_id": application_id.to_string(),
		"version": COMMAND_VERSION.to_string(),
		"name": "launch",
		"description": "Launch the activity",
		"handler": 2,
		"integration_types": [0],
		"contexts": [0, 1, 2],
	})
}

fn index(more_apps: bool) -> Value {
	let mut applications: Vec<Value> = APPLICATIONS
		.iter()
		.map(|(id, name, description)| application(*id, name, description))
		.collect();
	let mut commands: Vec<Value> = COMMAND_IDS
		.iter()
		.map(|(id, application_id, name)| command(*id, *application_id, name))
		.collect();
	if more_apps {
		applications.extend(
			MORE_APPLICATIONS
				.iter()
				.map(|(id, name, description)| application(*id, name, description)),
		);
		commands.extend(
			MORE_COMMANDS
				.iter()
				.map(|(id, application_id, name)| command(*id, *application_id, name)),
		);
		commands.push(entry_point(LAUNCH_COMMAND, ARCADE_ID));
	}
	json!({
		"applications": applications,
		"application_commands": commands,
		"version": COMMAND_VERSION.to_string(),
	})
}

pub fn frequent_commands(anchor: DateTime<Utc>) -> Vec<CommandUse> {
	command_uses(
		anchor,
		vec![
			(REMIND_COMMAND.to_string(), 12),
			(ROLL_COMMAND.to_string(), 7),
			(SHRUG_COMMAND.to_owned(), 3),
		],
	)
}

pub fn launcher_uses(anchor: DateTime<Utc>) -> Vec<CommandUse> {
	command_uses(
		anchor,
		vec![
			(ROLL_COMMAND.to_string(), 12),
			(REMIND_COMMAND.to_string(), 10),
			(POLL_COMMAND.to_string(), 8),
			(SNAP_COMMAND.to_string(), 6),
			(PLAY_COMMAND.to_string(), 4),
			(WARN_COMMAND.to_string(), 3),
		],
	)
}

fn command_uses(anchor: DateTime<Utc>, uses: Vec<(String, u32)>) -> Vec<CommandUse> {
	let now = anchor.timestamp_millis();
	uses.into_iter()
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
	let more_apps = shared.store.lock().more_apps;
	Ok(Json(index(more_apps)))
}

pub async fn channel_index(State(shared): State<Arc<Shared>>, Path(id): Path<u64>) -> ApiResult {
	if !shared.store.lock().channels.contains_key(&id) {
		return Err(ApiError::unknown_channel());
	}
	let more_apps = shared.store.lock().more_apps;
	Ok(Json(index(more_apps)))
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
	if body["session_id"].as_str().is_none_or(str::is_empty) {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	}
	match body["type"].as_i64() {
		Some(MESSAGE_COMPONENT) => return component(shared, &body),
		Some(MODAL_SUBMIT) => return modal_submit(&shared, &body),
		_ => {}
	}
	let data = &body["data"];
	let known = data["id"]
		.as_str()
		.and_then(|id| id.parse::<u64>().ok())
		.and_then(|id| COMMAND_IDS.iter().find(|(known, ..)| *known == id));
	let channel_id = body["channel_id"].as_str().and_then(|id| id.parse::<u64>().ok());
	let (Some((_, _, name)), Some(channel_id)) = (known, channel_id) else {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	};
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
		"bug" => {
			let modal = json!({
				"id": interaction_id,
				"channel_id": channel_id.to_string(),
				"custom_id": components::BUG_MODAL,
				"application": { "id": MOCKBOT_ID.to_string(), "name": "Mockbot", "icon": null },
				"title": components::BUG_TITLE,
				"components": components::bug_modal(),
				"nonce": nonce,
			});
			shared.hub.broadcast("INTERACTION_MODAL_CREATE", &modal);
			shared.hub.broadcast("INTERACTION_SUCCESS", &event);
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

fn parsed(value: &Value) -> Option<u64> {
	value.as_str().and_then(|id| id.parse().ok())
}

fn pressed_label(custom_id: &str) -> Option<&'static str> {
	match custom_id {
		"primary" => Some("Primary"),
		"secondary" => Some("Secondary"),
		"success" => Some("Success"),
		"celebrate" => Some("Celebrate"),
		_ => None,
	}
}

fn component(shared: Arc<Shared>, body: &Value) -> Result<StatusCode, ApiError> {
	let (Some(channel_id), Some(message_id), Some(custom_id)) = (
		parsed(&body["channel_id"]),
		parsed(&body["message_id"]),
		body["data"]["custom_id"].as_str(),
	) else {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	};
	let (known, interaction_id, timing) = {
		let mut store = shared.store.lock();
		let known = store
			.message(channel_id, message_id)
			.is_some_and(|message| parsed(&message["author"]["id"]) == Some(MOCKBOT_ID));
		(known, store.next_snowflake().to_string(), store.interactions)
	};
	if !known {
		return Err(ApiError::unknown_message());
	}
	let event = json!({ "id": interaction_id, "nonce": body["nonce"].clone() });
	shared.hub.broadcast("INTERACTION_CREATE", &event);
	match custom_id {
		"danger" => {
			tokio::spawn(async move {
				tokio::time::sleep(timing.window).await;
				shared.hub.broadcast("INTERACTION_FAILURE", &event);
			});
		}
		"refresh" => {
			tokio::spawn(async move {
				tokio::time::sleep(timing.followup).await;
				shared.hub.broadcast("INTERACTION_SUCCESS", &event);
			});
		}
		"fruit" | "fruits" => shared.hub.broadcast("INTERACTION_SUCCESS", &event),
		other => match pressed_label(other) {
			Some(label) => {
				shared.hub.broadcast("INTERACTION_SUCCESS", &event);
				shared.post_message(
					channel_id,
					&mockbot(),
					Draft::text(format!("{} {label}", components::PRESSED)),
				)?;
			}
			None => shared.hub.broadcast("INTERACTION_FAILURE", &event),
		},
	}
	Ok(StatusCode::NO_CONTENT)
}

fn modal_submit(shared: &Shared, body: &Value) -> Result<StatusCode, ApiError> {
	let data = &body["data"];
	let channel_id = parsed(&body["channel_id"]);
	let (Some(channel_id), Some(components::BUG_MODAL)) = (channel_id, data["custom_id"].as_str()) else {
		return Err(ApiError::invalid_body("Invalid Form Body"));
	};
	let title = components::submitted_text(&data["components"], "title")
		.ok_or(ApiError::invalid_body("Invalid Form Body"))?;
	let (interaction_id, failing) = {
		let mut store = shared.store.lock();
		(store.next_snowflake().to_string(), store.failing_modals)
	};
	let event = json!({ "id": interaction_id, "nonce": body["nonce"].clone() });
	shared.hub.broadcast("INTERACTION_CREATE", &event);
	if failing {
		shared.hub.broadcast("INTERACTION_FAILURE", &event);
		return Ok(StatusCode::NO_CONTENT);
	}
	shared.hub.broadcast("INTERACTION_SUCCESS", &event);
	shared.post_message(
		channel_id,
		&mockbot(),
		Draft::text(format!("{} {title}", components::BUG_FILED)),
	)?;
	Ok(StatusCode::NO_CONTENT)
}
