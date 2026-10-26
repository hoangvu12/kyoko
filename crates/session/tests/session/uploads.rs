use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mockcord::{Recording, Reply};
use protocol::{Permissions, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::{ExpressionKind, State};

use crate::harness::{Options, Running};

const GAP: Duration = Duration::from_millis(100);
const QUIET: Duration = Duration::from_millis(400);
const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff\x21\xf9\x04\x01\x00\x00\x00\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89";

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		guild_write_gap: GAP,
		..RestTuning::default()
	}
}

async fn start(name: &'static str) -> (Running, Snowflake) {
	let running = Running::start_with(Options::new(name).rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.until("a guild where I may add expressions", |state| {
			state.may_create_expressions(guild_id) && state.guild_stickers(guild_id).is_some()
		})
		.await;
	(running, guild_id)
}

fn posts(recording: &Recording, path: &str) -> Vec<mockcord::RestRequest> {
	recording.requests_to("POST", path).cloned().collect()
}

async fn rejected_with(running: &Running, what: &str, needle: &str) {
	let needle = needle.to_owned();
	running
		.until(what, move |state: &State| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.reason.contains(&needle))
		})
		.await;
}

struct Part {
	headers: String,
	body: Vec<u8>,
}

fn form_parts(request: &mockcord::RestRequest) -> Vec<Part> {
	let content_type = request.header("content-type").expect("a content type");
	assert!(content_type.starts_with("multipart/form-data"), "{content_type}");
	let boundary = content_type
		.split("boundary=")
		.nth(1)
		.expect("a boundary")
		.trim_matches('"');
	let delimiter = format!("--{boundary}");
	let body = &request.body;
	let mut parts = Vec::new();
	let mut rest: &[u8] = body;
	while let Some(start) = find(rest, delimiter.as_bytes()) {
		rest = &rest[start + delimiter.len()..];
		if rest.starts_with(b"--") {
			break;
		}
		let rest_trimmed = rest.strip_prefix(b"\r\n").unwrap_or(rest);
		let Some(split) = find(rest_trimmed, b"\r\n\r\n") else {
			break;
		};
		let headers = String::from_utf8_lossy(&rest_trimmed[..split]).into_owned();
		let content = &rest_trimmed[split + 4..];
		let end = find(content, delimiter.as_bytes()).expect("the part ends");
		parts.push(Part {
			headers,
			body: content[..end]
				.strip_suffix(b"\r\n")
				.unwrap_or(&content[..end])
				.to_vec(),
		});
		rest = &content[end..];
	}
	parts
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
	haystack.windows(needle.len()).position(|window| window == needle)
}

fn field<'a>(parts: &'a [Part], name: &str) -> Option<&'a Part> {
	let marker = format!("name=\"{name}\"");
	parts.iter().find(|part| part.headers.contains(&marker))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_emoji_upload_sends_the_web_clients_body_and_arrives() {
	let (running, guild_id) = start("uploads-emoji").await;
	running.session().send(Command::CreateEmoji {
		guild_id,
		name: "party_parrot".to_owned(),
		image: GIF.to_vec(),
	});
	let path = format!("/guilds/{guild_id}/emojis");
	let body = running
		.until_wire("the emoji upload", |recording| {
			posts(recording, &path).pop()?.json()
		})
		.await;
	let image = format!("data:image/gif;base64,{}", STANDARD.encode(GIF));
	assert_eq!(body, json!({ "name": "party_parrot", "image": image }));
	running
		.until("the cloned emoji in the server", |state: &State| {
			state.guild(guild_id).is_some_and(|guild| {
				guild.emojis.iter().any(|emoji| {
					emoji.name.as_deref() == Some("party_parrot") && emoji.animated == Some(true)
				})
			})
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sticker_upload_is_a_multipart_form_with_its_file() {
	let (running, guild_id) = start("uploads-sticker").await;
	running.session().send(Command::CreateSticker {
		guild_id,
		name: "wave hello".to_owned(),
		description: "A friendly wave".to_owned(),
		tags: "wave".to_owned(),
		file: PNG.to_vec(),
	});
	let path = format!("/guilds/{guild_id}/stickers");
	let request = running
		.until_wire("the sticker upload", |recording| posts(recording, &path).pop())
		.await;
	let parts = form_parts(&request);
	let text = |name: &str| {
		field(&parts, name)
			.map(|part| String::from_utf8_lossy(&part.body).into_owned())
			.unwrap_or_default()
	};
	assert_eq!(text("name"), "wave hello");
	assert_eq!(text("description"), "A friendly wave");
	assert_eq!(text("tags"), "wave");
	let file = field(&parts, "file").expect("a file part");
	assert!(
		file.headers.contains("filename=\"wave hello.png\""),
		"{}",
		file.headers
	);
	assert!(
		file.headers
			.to_ascii_lowercase()
			.contains("content-type: image/png"),
		"{}",
		file.headers
	);
	assert_eq!(file.body, PNG);
	assert_eq!(parts.len(), 4);
	running
		.until("the cloned sticker in the server", |state: &State| {
			state
				.guild_stickers(guild_id)
				.is_some_and(|stickers| stickers.iter().any(|sticker| sticker.name == "wave hello"))
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uploads_without_create_expressions_never_reach_rest() {
	let (running, guild_id) = start("uploads-permission").await;
	let user = running.fixtures().current_user.clone();
	running.mock().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({ "guild_id": guild_id.to_string(), "user": user, "roles": [], "joined_at": "2025-01-01T00:00:00+00:00" }),
	);
	running
		.until("the current user without roles", |state: &State| {
			!state.may_create_expressions(guild_id)
		})
		.await;
	let me = running
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("signed in");
	assert!(
		!running
			.session()
			.state()
			.read()
			.guild_permissions(guild_id, me)
			.intersects(Permissions::CREATE_GUILD_EXPRESSIONS | Permissions::MANAGE_GUILD_EXPRESSIONS)
	);
	running.session().send(Command::CreateEmoji {
		guild_id,
		name: "refused".to_owned(),
		image: PNG.to_vec(),
	});
	rejected_with(&running, "the emoji refusal", "Create Expressions").await;
	running.session().send(Command::CreateSticker {
		guild_id,
		name: "refused".to_owned(),
		description: String::new(),
		tags: "refused".to_owned(),
		file: PNG.to_vec(),
	});
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	assert!(posts(&recording, &format!("/guilds/{guild_id}/emojis")).is_empty());
	assert!(posts(&recording, &format!("/guilds/{guild_id}/stickers")).is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_out_of_slots_refuses_before_rest() {
	let (running, guild_id) = start("uploads-slots").await;
	let (emoji_max, sticker_max) = {
		let state = running.session().state().read();
		(
			state
				.expression_slots(guild_id, ExpressionKind::Emoji)
				.expect("the guild")
				.max,
			state
				.expression_slots(guild_id, ExpressionKind::Sticker)
				.expect("the guild")
				.max,
		)
	};
	let emojis: Vec<Value> = (0..emoji_max)
		.map(|index| json!({ "id": (9_000_000 + index).to_string(), "name": format!("still_{index}"), "animated": false }))
		.collect();
	running.mock().dispatch(
		"GUILD_EMOJIS_UPDATE",
		&json!({ "guild_id": guild_id.to_string(), "emojis": emojis }),
	);
	let stickers: Vec<Value> = (0..sticker_max)
		.map(|index| json!({ "id": (8_000_000 + index).to_string(), "name": format!("sticker {index}"), "format_type": 1, "type": 2, "guild_id": guild_id.to_string() }))
		.collect();
	running.mock().dispatch(
		"GUILD_STICKERS_UPDATE",
		&json!({ "guild_id": guild_id.to_string(), "stickers": stickers }),
	);
	running
		.until("a full server", |state: &State| {
			state
				.expression_slots(guild_id, ExpressionKind::Emoji)
				.is_some_and(state::Slots::full)
				&& state
					.expression_slots(guild_id, ExpressionKind::Sticker)
					.is_some_and(state::Slots::full)
		})
		.await;
	running.session().send(Command::CreateEmoji {
		guild_id,
		name: "one_too_many".to_owned(),
		image: PNG.to_vec(),
	});
	rejected_with(&running, "the emoji slot refusal", "no free emoji slots").await;
	running.session().send(Command::CreateSticker {
		guild_id,
		name: "one too many".to_owned(),
		description: String::new(),
		tags: "many".to_owned(),
		file: PNG.to_vec(),
	});
	rejected_with(&running, "the sticker slot refusal", "no free sticker slots").await;
	running.session().send(Command::CreateEmoji {
		guild_id,
		name: "animated_fits".to_owned(),
		image: GIF.to_vec(),
	});
	let path = format!("/guilds/{guild_id}/emojis");
	let body = running
		.until_wire("the animated emoji, which has its own slots", |recording| {
			posts(recording, &path).pop()?.json()
		})
		.await;
	assert_eq!(body["name"], "animated_fits");
	let recording = running.mock().recording();
	assert_eq!(posts(&recording, &path).len(), 1);
	assert!(posts(&recording, &format!("/guilds/{guild_id}/stickers")).is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_names_files_and_sizes_are_refused_before_rest() {
	let (running, guild_id) = start("uploads-validation").await;
	let cases = [
		(
			Command::CreateEmoji {
				guild_id,
				name: "x".to_owned(),
				image: PNG.to_vec(),
			},
			"2 to 32 characters",
		),
		(
			Command::CreateEmoji {
				guild_id,
				name: "has space".to_owned(),
				image: PNG.to_vec(),
			},
			"letters, numbers and underscores",
		),
		(
			Command::CreateEmoji {
				guild_id,
				name: "not_an_image".to_owned(),
				image: b"<svg/>".to_vec(),
			},
			"PNG, GIF, JPEG or WebP",
		),
		(
			Command::CreateEmoji {
				guild_id,
				name: "too_big".to_owned(),
				image: [PNG, &vec![0; 256 * 1024]].concat(),
			},
			"at most 256 KB",
		),
		(
			Command::CreateSticker {
				guild_id,
				name: "no tags".to_owned(),
				description: String::new(),
				tags: " ".to_owned(),
				file: PNG.to_vec(),
			},
			"tags",
		),
		(
			Command::CreateSticker {
				guild_id,
				name: "jpeg".to_owned(),
				description: String::new(),
				tags: "jpeg".to_owned(),
				file: b"\xff\xd8\xff\xe0".to_vec(),
			},
			"PNG, APNG or GIF",
		),
	];
	for (command, reason) in cases {
		running.session().send(command);
		rejected_with(&running, reason, reason).await;
	}
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	assert!(posts(&recording, &format!("/guilds/{guild_id}/emojis")).is_empty());
	assert!(posts(&recording, &format!("/guilds/{guild_id}/stickers")).is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_upload_comes_back_as_a_rejection() {
	let (running, guild_id) = start("uploads-refused").await;
	let path = format!("/guilds/{guild_id}/emojis");
	running.mock().respond_once(
		"POST",
		&path,
		Reply::json(
			400,
			json!({ "code": 30008, "message": "Maximum number of emojis reached (50)" }),
		),
	);
	running.session().send(Command::CreateEmoji {
		guild_id,
		name: "refused_by_discord".to_owned(),
		image: PNG.to_vec(),
	});
	running
		.until_wire("the refused upload", |recording| posts(recording, &path).pop())
		.await;
	rejected_with(&running, "the upload failure", "Create emoji").await;
	running.stop().await;
}
