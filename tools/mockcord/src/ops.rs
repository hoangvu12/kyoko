use chrono::{SecondsFormat, Utc};
use protocol::{
	Attachment, AttachmentDescriptor, CreateMessage, EditMessage, Embed, EmbedKind, EmbedMedia,
	EmbedProvider, LenientVec, Message, MessageFlags, MessageReference, MessageReferenceKind, MessageType,
	Nonce, ReadState, ReadStateKind, Snowflake, StickerFormat, StickerItem, User,
};
use serde_json::{Value, json};

use crate::cdn::{self, Signature};
use crate::error::ApiError;
use crate::fixtures::CURRENT_USER_ID;
use crate::forward::snapshot_of;
use crate::json::{author_id, parse_id, to_json};
use crate::state::{Shared, Store};

pub struct UploadedFile {
	pub filename: String,
	pub content_type: Option<String>,
	pub size: usize,
}

#[derive(Default)]
pub struct Draft {
	pub id: Option<u64>,
	pub kind: Option<MessageType>,
	pub content: String,
	pub nonce: Option<String>,
	pub tts: bool,
	pub reference: Option<MessageReference>,
	pub flags: MessageFlags,
	pub sticker_ids: Vec<Snowflake>,
	pub descriptors: Vec<AttachmentDescriptor>,
	pub files: Vec<UploadedFile>,
	pub hosted: Option<Signature>,
}

impl Draft {
	pub fn text(content: impl Into<String>) -> Self {
		Self {
			content: content.into(),
			..Self::default()
		}
	}

	pub fn from_create(create: CreateMessage, files: Vec<UploadedFile>) -> Self {
		Self {
			id: None,
			kind: None,
			content: create.content.unwrap_or_default(),
			nonce: create.nonce,
			tts: create.tts.unwrap_or(false),
			reference: create.message_reference,
			flags: create.flags.unwrap_or_default(),
			sticker_ids: create.sticker_ids,
			descriptors: create.attachments,
			files,
			hosted: None,
		}
	}
}

pub fn now_iso() -> String {
	Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn scan_mentions(content: &str) -> (Vec<u64>, Vec<u64>) {
	let mut users = Vec::new();
	let mut roles = Vec::new();
	let mut rest = content;
	while let Some(start) = rest.find("<@") {
		let tail = &rest[start + 2..];
		let (is_role, tail) = match tail.strip_prefix('&') {
			Some(stripped) => (true, stripped),
			None => (false, tail.strip_prefix('!').unwrap_or(tail)),
		};
		let digits = tail.chars().take_while(|c| c.is_ascii_digit()).count();
		if digits > 0
			&& tail[digits..].starts_with('>')
			&& let Ok(id) = tail[..digits].parse::<u64>()
		{
			if is_role {
				roles.push(id);
			} else {
				users.push(id);
			}
		}
		rest = &rest[start + 2..];
	}
	(users, roles)
}

const GIF_WIDTH: i64 = 498;
const GIF_HEIGHT: i64 = 280;

fn unfurled(content: &str) -> Vec<Embed> {
	content
		.split_whitespace()
		.filter(|word| word.starts_with("https://") && !word.starts_with("<"))
		.filter_map(|link| {
			let rest = link.strip_prefix("https://")?;
			let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
			let host = host.trim_start_matches("www.").to_owned();
			if host.is_empty() {
				return None;
			}
			let title = path
				.split(['?', '#'])
				.next()
				.unwrap_or_default()
				.trim_matches('/')
				.to_owned();
			if let Some(provider) = gif_provider(&host) {
				return Some(gif_embed(link, provider, &title));
			}
			Some(Embed {
				kind: Some(EmbedKind::Link),
				url: Some(link.to_owned()),
				title: Some(if title.is_empty() { host.clone() } else { title }),
				description: Some(format!("A page on {host}.")),
				provider: Some(EmbedProvider {
					name: Some(host),
					url: None,
				}),
				..Embed::default()
			})
		})
		.take(1)
		.collect()
}

fn gif_provider(host: &str) -> Option<&'static str> {
	match host {
		"klipy.com" => Some("Klipy"),
		"tenor.com" => Some("Tenor"),
		_ => None,
	}
}

fn gif_embed(link: &str, provider: &str, path: &str) -> Embed {
	let slug = path.rsplit('/').next().unwrap_or(path);
	let host = provider.to_lowercase();
	let media = |extension: &str| EmbedMedia {
		url: Some(format!("https://static.{host}.com/{slug}.{extension}")),
		width: Some(GIF_WIDTH),
		height: Some(GIF_HEIGHT),
		..EmbedMedia::default()
	};
	Embed {
		kind: Some(EmbedKind::Gifv),
		url: Some(link.to_owned()),
		provider: Some(EmbedProvider {
			name: Some(provider.to_owned()),
			url: None,
		}),
		thumbnail: Some(media("png")),
		video: Some(media("mp4")),
		..Embed::default()
	}
}

fn sanitized_filename(name: &str) -> String {
	name.chars()
		.map(|c| {
			if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
				c
			} else {
				'_'
			}
		})
		.collect()
}

impl Shared {
	pub fn partial_member(&self, guild_id: u64, user_id: u64) -> Value {
		let Some(member) = self
			.fixtures
			.guild(guild_id)
			.and_then(|guild| guild.member(user_id))
		else {
			return Value::Null;
		};
		let mut value = to_json(member);
		if let Some(fields) = value.as_object_mut() {
			fields.remove("user");
		}
		value
	}

	pub fn gateway_message(&self, store: &Store, message: &Value) -> Value {
		let mut value = message.clone();
		let guild_id = parse_id(&message["channel_id"])
			.and_then(|channel_id| store.channels.get(&channel_id))
			.and_then(|channel| channel.guild_id);
		if let Some(guild_id) = guild_id {
			value["guild_id"] = json!(guild_id.to_string());
			value["member"] = self.partial_member(guild_id.0, author_id(message));
		}
		value
	}

	pub(crate) fn guild_of(store: &Store, channel_id: u64) -> Result<Option<Snowflake>, ApiError> {
		store
			.channels
			.get(&channel_id)
			.map(|channel| channel.guild_id)
			.ok_or_else(ApiError::unknown_channel)
	}

	pub fn post_message(&self, channel_id: u64, author: &User, draft: Draft) -> Result<Value, ApiError> {
		let forward = draft
			.reference
			.as_ref()
			.is_some_and(|reference| reference.kind == Some(MessageReferenceKind::Forward));
		if draft.content.is_empty() && draft.files.is_empty() && draft.sticker_ids.is_empty() && !forward {
			return Err(ApiError::empty_message());
		}
		let mut store = self.store.lock();
		let guild_id = Self::guild_of(&store, channel_id)?;
		let id = draft.id.unwrap_or_else(|| store.next_snowflake());
		let (user_ids, mention_roles) = scan_mentions(&draft.content);
		let mentions: Vec<User> = user_ids
			.iter()
			.filter_map(|user_id| self.fixtures.users.get(user_id).cloned())
			.collect();
		let mention_everyone = draft.content.contains("@everyone") || draft.content.contains("@here");
		let embeds = unfurled(&draft.content);
		let mut attachments = Vec::with_capacity(draft.files.len());
		for (index, file) in draft.files.iter().enumerate() {
			let attachment_id = store.next_snowflake();
			let mut attachment =
				uploaded_attachment(channel_id, attachment_id, index, file, &draft.descriptors);
			if let Some(signature) = draft.hosted {
				host_attachment(&mut attachment, &self.urls.origin, channel_id, signature);
			}
			attachments.push(attachment);
		}
		let mut message = Message {
			id: Snowflake(id),
			channel_id: Snowflake(channel_id),
			author: author.clone(),
			content: draft.content,
			timestamp: Some(Utc::now()),
			tts: draft.tts,
			mention_everyone,
			mentions: LenientVec::from(mentions),
			mention_roles: mention_roles.into_iter().map(Snowflake).collect(),
			attachments: LenientVec::from(attachments),
			nonce: draft.nonce.map(Nonce),
			flags: draft.flags,
			kind: draft.kind.unwrap_or_default(),
			embeds: LenientVec::from(embeds),
			sticker_items: LenientVec::from(
				draft
					.sticker_ids
					.iter()
					.map(|sticker_id| StickerItem {
						id: *sticker_id,
						name: "Wave".to_owned(),
						format_type: Some(StickerFormat::Png),
					})
					.collect::<Vec<_>>(),
			),
			..Message::default()
		};
		if let Some(reference) = draft.reference
			&& let Some(target) = reference.message_id
		{
			let source_channel = reference.channel_id.map_or(channel_id, Snowflake::get);
			let found = store
				.message(source_channel, target.0)
				.cloned()
				.and_then(|mut value| {
					value["referenced_message"] = Value::Null;
					serde_json::from_value::<Message>(value).ok()
				});
			match (found, reference.fail_if_not_exists) {
				(Some(source), _) => {
					if reference.kind == Some(MessageReferenceKind::Forward) {
						message.message_snapshots = LenientVec::from(vec![snapshot_of(source)]);
						message.flags |= MessageFlags::HAS_SNAPSHOT;
					} else {
						if message.kind == MessageType::Default {
							message.kind = MessageType::Reply;
						}
						message.referenced_message = Some(Box::new(source));
					}
					message.message_reference = Some(MessageReference {
						channel_id: Some(Snowflake(source_channel)),
						guild_id: guild_id.or(reference.guild_id),
						..reference
					});
				}
				(None, Some(false)) => {}
				(None, _) => return Err(ApiError::unknown_message()),
			}
		}
		let value = to_json(&message);
		store
			.messages
			.entry(channel_id)
			.or_default()
			.insert(id, value.clone());
		store.refresh_last_message(channel_id);
		let from_me = author.id.0 == CURRENT_USER_ID;
		let pings_me =
			message.mention_everyone || message.mentions.iter().any(|user| user.id.0 == CURRENT_USER_ID);
		if from_me {
			let state = store
				.read_states
				.entry(channel_id)
				.or_insert_with(|| blank_read_state(channel_id));
			state.last_message_id = Some(Snowflake(id));
			state.mention_count = Some(0);
			store.read_state_version.bump(channel_id);
		} else if pings_me && let Some(state) = store.read_states.get_mut(&channel_id) {
			state.mention_count = Some(state.mention_count.unwrap_or(0) + 1);
			store.read_state_version.bump(channel_id);
		}
		let gateway = self.gateway_message(&store, &value);
		drop(store);
		self.hub.broadcast("MESSAGE_CREATE", &gateway);
		Ok(value)
	}

	pub fn edit_message(&self, channel_id: u64, id: u64, edit: EditMessage) -> Result<Value, ApiError> {
		self.edit_as(channel_id, id, edit, Some(CURRENT_USER_ID))
	}

	pub fn edit_any_message(&self, channel_id: u64, id: u64, content: &str) -> Result<Value, ApiError> {
		let edit = EditMessage {
			content: Some(content.to_owned()),
			..EditMessage::default()
		};
		self.edit_as(channel_id, id, edit, None)
	}

	fn edit_as(
		&self,
		channel_id: u64,
		id: u64,
		edit: EditMessage,
		editor: Option<u64>,
	) -> Result<Value, ApiError> {
		let mut store = self.store.lock();
		Self::guild_of(&store, channel_id)?;
		let message = store
			.message_mut(channel_id, id)
			.ok_or_else(ApiError::unknown_message)?;
		if editor.is_some_and(|editor| author_id(message) != editor) {
			return Err(ApiError::not_author());
		}
		let edited = edit.content.is_some() || edit.attachments.is_some();
		if let Some(content) = edit.content {
			message["content"] = Value::String(content);
		}
		if let Some(flags) = edit.flags {
			message["flags"] = json!(flags.bits());
		}
		if let (Some(keep), Some(attachments)) = (edit.attachments, message["attachments"].as_array_mut()) {
			attachments.retain(|attachment| {
				parse_id(&attachment["id"])
					.is_some_and(|attachment_id| keep.iter().any(|kept| kept.id.0 == attachment_id))
			});
		}
		if edited {
			message["edited_timestamp"] = Value::String(now_iso());
		}
		let value = message.clone();
		let gateway = self.gateway_message(&store, &value);
		drop(store);
		self.hub.broadcast("MESSAGE_UPDATE", &gateway);
		Ok(value)
	}

	pub fn delete_message(&self, channel_id: u64, id: u64) -> Result<(), ApiError> {
		let mut store = self.store.lock();
		let guild_id = Self::guild_of(&store, channel_id)?;
		store
			.messages
			.get_mut(&channel_id)
			.and_then(|messages| messages.remove(&id))
			.ok_or_else(ApiError::unknown_message)?;
		store.refresh_last_message(channel_id);
		drop(store);
		let mut data = json!({ "id": id.to_string(), "channel_id": channel_id.to_string() });
		if let Some(guild_id) = guild_id {
			data["guild_id"] = json!(guild_id.to_string());
		}
		self.hub.broadcast("MESSAGE_DELETE", &data);
		Ok(())
	}

	pub fn typing(&self, channel_id: u64, user_id: u64) -> Result<(), ApiError> {
		let guild_id = Self::guild_of(&self.store.lock(), channel_id)?;
		let mut data = json!({
			"channel_id": channel_id.to_string(),
			"user_id": user_id.to_string(),
			"timestamp": Utc::now().timestamp(),
		});
		if let Some(guild_id) = guild_id {
			data["guild_id"] = json!(guild_id.to_string());
			data["member"] = self.partial_member(guild_id.0, user_id);
			data["member"]["user"] = self.fixtures.users.get(&user_id).map_or(Value::Null, to_json);
		}
		self.hub.broadcast("TYPING_START", &data);
		Ok(())
	}

	pub fn ack(&self, channel_id: u64, id: u64, mention_count: Option<i64>) -> Result<(), ApiError> {
		let mut store = self.store.lock();
		Self::guild_of(&store, channel_id)?;
		let count = mention_count.unwrap_or(0);
		let state = store
			.read_states
			.entry(channel_id)
			.or_insert_with(|| blank_read_state(channel_id));
		state.last_message_id = Some(Snowflake(id));
		state.mention_count = Some(count);
		let version = store.read_state_version.bump(channel_id);
		drop(store);
		let data = json!({
			"channel_id": channel_id.to_string(),
			"message_id": id.to_string(),
			"version": version,
			"mention_count": count,
			"ack_type": 0,
			"flags": 0,
		});
		self.hub.broadcast("MESSAGE_ACK", &data);
		Ok(())
	}
}

fn blank_read_state(channel_id: u64) -> ReadState {
	ReadState {
		id: Snowflake(channel_id),
		flags: Some(0),
		mention_count: Some(0),
		read_state_type: Some(ReadStateKind::Channel),
		..ReadState::default()
	}
}

fn uploaded_attachment(
	channel_id: u64,
	attachment_id: u64,
	index: usize,
	file: &UploadedFile,
	descriptors: &[AttachmentDescriptor],
) -> Attachment {
	let filename = sanitized_filename(&file.filename);
	let descriptor = descriptors
		.iter()
		.find(|descriptor| descriptor.id.0 == index as u64);
	Attachment {
		id: Snowflake(attachment_id),
		url: Some(format!(
			"https://cdn.discordapp.com/attachments/{channel_id}/{attachment_id}/{filename}"
		)),
		proxy_url: Some(format!(
			"https://media.discordapp.net/attachments/{channel_id}/{attachment_id}/{filename}"
		)),
		filename,
		title: descriptor.and_then(|descriptor| descriptor.title.clone()),
		description: descriptor.and_then(|descriptor| descriptor.description.clone()),
		content_type: file.content_type.clone(),
		size: Some(file.size as i64),
		duration_secs: descriptor.and_then(|descriptor| descriptor.duration_secs),
		waveform: descriptor.and_then(|descriptor| descriptor.waveform.clone()),
		flags: Some(0),
		..Attachment::default()
	}
}

fn host_attachment(attachment: &mut Attachment, origin: &str, channel_id: u64, signature: Signature) {
	let path = cdn::attachment_path(channel_id, attachment.id.0, &attachment.filename);
	let url = format!("{origin}{path}?{}", cdn::signed_query(&path, signature));
	let (width, height) = cdn::attachment_size();
	attachment.url = Some(url.clone());
	attachment.proxy_url = Some(url);
	if !crate::audio::is_audio(&attachment.filename) {
		attachment.width = Some(i64::from(width));
		attachment.height = Some(i64::from(height));
	}
}
