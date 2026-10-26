use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use protocol::rest::{AllowedMentions, AttachmentDescriptor};
use protocol::{
	Channel, CreateMessage, Message, MessageReference, MessageReferenceKind, Nonce, Permissions, Snowflake,
	StickerItem,
};
use rest::api::UploadSlotRequest;
use rest::{RestClient, RestError, UploadFile};
use serde_json::{Value, json};
use state::{CommandStage, LocalEvent, MAX_ATTACHMENTS, PendingFile, PendingMessage, SendFailure};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use crate::hooks::{Hooks, Rewrite, guarded_blocking};
use crate::poll::PollDraft;

use super::actions::failure_reason;
use super::{Core, Internal, failures};

const KEPT_FAILURES: usize = 16;
const MIB: f64 = 1024.0 * 1024.0;
const PROGRESS_STEPS: u64 = 100;

#[derive(Debug, Clone)]
pub(crate) struct Outgoing {
	pub(crate) channel_id: Snowflake,
	pub(crate) content: String,
	pub(crate) reply_to: Option<Snowflake>,
	pub(crate) ping_reply: bool,
	pub(crate) files: Vec<UploadFile>,
	pub(crate) stickers: Vec<StickerItem>,
	pub(crate) poll: Option<PollDraft>,
}

#[derive(Debug, Clone)]
pub(crate) struct ForumPost {
	pub(crate) channel_id: Snowflake,
	pub(crate) name: String,
	pub(crate) content: String,
	pub(crate) tags: Vec<Snowflake>,
	pub(crate) files: Vec<UploadFile>,
	pub(crate) auto_archive_minutes: i64,
}

#[derive(Default)]
pub(crate) struct Outbox {
	failed: VecDeque<(Nonce, Outgoing)>,
	uploading: Vec<(Nonce, AbortHandle)>,
}

impl Outbox {
	fn uploading(&mut self, nonce: Nonce, handle: AbortHandle) {
		self.uploading.retain(|(_, running)| !running.is_finished());
		self.uploading.push((nonce, handle));
	}

	fn cancel(&mut self, nonce: &Nonce) -> bool {
		let Some(index) = self.uploading.iter().position(|(running, _)| running == nonce) else {
			return false;
		};
		let (_, handle) = self.uploading.swap_remove(index);
		let running = !handle.is_finished();
		handle.abort();
		running
	}

	fn keep(&mut self, nonce: Nonce, outgoing: Outgoing) {
		self.failed.retain(|(kept, _)| *kept != nonce);
		if self.failed.len() >= KEPT_FAILURES {
			self.failed.pop_front();
		}
		self.failed.push_back((nonce, outgoing));
	}

	fn take(&mut self, nonce: &Nonce) -> Option<Outgoing> {
		let index = self.failed.iter().position(|(kept, _)| kept == nonce)?;
		self.failed.remove(index).map(|(_, outgoing)| outgoing)
	}
}

fn create_body(nonce: &Nonce, outgoing: &Outgoing) -> CreateMessage {
	let channel_id = outgoing.channel_id;
	CreateMessage {
		content: (!outgoing.content.is_empty()).then(|| outgoing.content.clone()),
		nonce: Some(nonce.0.clone()),
		message_reference: outgoing.reply_to.map(|message_id| MessageReference {
			message_id: Some(message_id),
			channel_id: Some(channel_id),
			fail_if_not_exists: Some(false),
			..MessageReference::default()
		}),
		allowed_mentions: (outgoing.reply_to.is_some() && !outgoing.ping_reply).then(|| AllowedMentions {
			parse: Some(vec![
				"users".to_owned(),
				"roles".to_owned(),
				"everyone".to_owned(),
			]),
			replied_user: Some(false),
			..AllowedMentions::default()
		}),
		sticker_ids: outgoing.stickers.iter().map(|sticker| sticker.id).collect(),
		poll: outgoing.poll.as_ref().map(PollDraft::body),
		..CreateMessage::default()
	}
}

type Progress = Arc<dyn Fn(u64) + Send + Sync>;

fn progress_of(nonce: Nonce, internal: UnboundedSender<Internal>, total: u64) -> Progress {
	let reported = Arc::new(AtomicU64::new(u64::MAX));
	Arc::new(move |sent: u64| {
		let step = sent.saturating_mul(PROGRESS_STEPS) / total.max(1);
		if reported.swap(step, Ordering::Relaxed) != step {
			let _ = internal.send(Internal::Uploading(nonce.clone(), sent));
		}
	})
}

pub(super) async fn uploaded(
	rest: &RestClient,
	files: &[UploadFile],
	channel_id: Snowflake,
	progress: Option<Progress>,
) -> Result<Vec<AttachmentDescriptor>, RestError> {
	let requests: Vec<UploadSlotRequest> = files
		.iter()
		.enumerate()
		.map(|(index, file)| UploadSlotRequest {
			id: index.to_string(),
			filename: file.upload_name(),
			file_size: file.size(),
			is_clip: false,
		})
		.collect();
	let slots = rest.attachment_slots(channel_id, &requests).await?;
	let mut attachments = Vec::with_capacity(files.len());
	let mut before = 0;
	for (index, file) in files.iter().enumerate() {
		let slot = slots
			.iter()
			.find(|slot| slot.id == index as i64)
			.ok_or(RestError::InvalidRequest)?;
		let report = progress.clone();
		rest.upload(&slot.upload_url, file, move |sent| {
			if let Some(report) = &report {
				report(before + sent);
			}
		})
		.await?;
		before += file.size();
		attachments.push(AttachmentDescriptor {
			id: Snowflake(index as u64),
			filename: Some(file.upload_name()),
			uploaded_filename: Some(slot.upload_filename.clone()),
			description: file.description.clone(),
			..AttachmentDescriptor::default()
		});
	}
	Ok(attachments)
}

async fn upload_and_send(
	rest: &RestClient,
	mut body: CreateMessage,
	files: &[UploadFile],
	channel_id: Snowflake,
	progress: Progress,
) -> Result<Message, RestError> {
	body.attachments = uploaded(rest, files, channel_id, Some(progress)).await?;
	rest.create_message(channel_id, body).await
}

fn post_body(post: &ForumPost, content: String, attachments: Vec<AttachmentDescriptor>) -> Value {
	let mut message = json!({ "content": content });
	if !attachments.is_empty() {
		message["attachments"] = json!(attachments);
	}
	json!({
		"name": post.name,
		"auto_archive_duration": post.auto_archive_minutes,
		"applied_tags": post.tags,
		"message": message,
	})
}

async fn create_post(rest: &RestClient, hooks: &Hooks, post: ForumPost) -> Result<Channel, RestError> {
	let channel_id = post.channel_id;
	let original = post.content.clone();
	let content = match hooks {
		Some(_) => guarded_blocking(hooks, "before_send", move |hooks| {
			hooks.before_send(channel_id, &original)
		})
		.await
		.flatten()
		.map_or_else(|| post.content.clone(), |rewrite| rewrite.content),
		None => post.content.clone(),
	};
	let attachments = if post.files.is_empty() {
		Vec::new()
	} else {
		uploaded(rest, &post.files, channel_id, None).await?
	};
	rest.create_forum_post(channel_id, post_body(&post, content, attachments))
		.await
}

pub(crate) fn too_large(files: &[UploadFile], limit: u64) -> Option<String> {
	if files.len() > MAX_ATTACHMENTS {
		return Some(format!("You can attach up to {MAX_ATTACHMENTS} files"));
	}
	let file = files.iter().find(|file| file.bytes.len() as u64 > limit)?;
	Some(format!(
		"{} is larger than the {} MB upload limit",
		file.name,
		(limit as f64 / MIB).round()
	))
}

impl Core {
	pub(super) fn send_message(&mut self, outgoing: Outgoing) {
		let Some(author) = self.current_user() else {
			tracing::warn!("message not sent, no signed in user");
			return;
		};
		if outgoing.content.trim().is_empty()
			&& outgoing.files.is_empty()
			&& outgoing.stickers.is_empty()
			&& outgoing.poll.is_none()
		{
			return;
		}
		let limit = self.state.read().upload_limit(outgoing.channel_id);
		if let Some(reason) = too_large(&outgoing.files, limit) {
			self.reject(reason);
			return;
		}
		self.typing_sent.remove(&outgoing.channel_id);
		self.remember_message_emoji(&outgoing.content);
		let nonce = self.next_nonce();
		self.apply_local(LocalEvent::PendingMessage {
			channel_id: outgoing.channel_id,
			nonce: nonce.clone(),
			content: outgoing.content.clone(),
			stickers: outgoing.stickers.clone(),
			poll: outgoing
				.poll
				.as_ref()
				.map(|poll| Box::new(poll.preview(Utc::now()))),
			author: Box::new(author),
			by_plugin: None,
			files: outgoing
				.files
				.iter()
				.map(|file| PendingFile {
					name: file.upload_name(),
					size: file.size(),
				})
				.collect(),
		});
		if self.hooks.is_some() && !outgoing.content.is_empty() {
			self.hook_before_send(nonce, outgoing);
		} else {
			self.dispatch_send(nonce, outgoing);
		}
	}

	fn hook_before_send(&self, nonce: Nonce, outgoing: Outgoing) {
		let hooks: Hooks = self.hooks.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let channel_id = outgoing.channel_id;
			let original = outgoing.content.clone();
			let rewritten = guarded_blocking(&hooks, "before_send", move |hooks| {
				hooks.before_send(channel_id, &original)
			})
			.await
			.flatten()
			.filter(|rewritten| rewritten.content != outgoing.content);
			let _ = internal.send(Internal::BeforeSend {
				nonce,
				outgoing: Box::new(outgoing),
				rewritten,
			});
		});
	}

	pub(super) fn on_before_send(
		&mut self,
		nonce: Nonce,
		mut outgoing: Outgoing,
		rewritten: Option<Rewrite>,
	) {
		if let Some(rewritten) = rewritten {
			let by = rewritten.by.unwrap_or_else(|| {
				self.hooks
					.as_deref()
					.map_or_else(String::new, |hooks| hooks.name().to_owned())
			});
			self.apply_local(LocalEvent::PendingAltered {
				nonce: nonce.clone(),
				content: rewritten.content.clone(),
				by,
			});
			outgoing.content = rewritten.content;
		}
		self.dispatch_send(nonce, outgoing);
	}

	fn dispatch_send(&mut self, nonce: Nonce, outgoing: Outgoing) {
		let body = create_body(&nonce, &outgoing);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		let uploads = !outgoing.files.is_empty();
		let total = outgoing.files.iter().map(UploadFile::size).sum();
		let progress = progress_of(nonce.clone(), internal.clone(), total);
		let sending = nonce.clone();
		let task = self.runtime.spawn(async move {
			let channel_id = outgoing.channel_id;
			let sent = if outgoing.files.is_empty() {
				rest.create_message(channel_id, body).await
			} else {
				upload_and_send(&rest, body, &outgoing.files, channel_id, progress).await
			};
			if let Err(error) = sent {
				tracing::warn!(%error, "message was not sent");
				let failure = failures::send_failure(&error);
				let _ = internal.send(Internal::SendFailed(sending, failure, Box::new(outgoing)));
			}
		});
		if uploads {
			self.outbox.uploading(nonce, task.abort_handle());
		}
	}

	pub(super) fn send_poll(&mut self, channel_id: Snowflake, poll: PollDraft) {
		if let Some(problem) = poll.problem() {
			self.reject(problem);
			return;
		}
		let allowed = self
			.state
			.read()
			.my_permissions(channel_id)
			.contains(Permissions::SEND_MESSAGES | Permissions::SEND_POLLS);
		if !allowed {
			self.reject("You can't create polls in this channel");
			return;
		}
		self.send_message(Outgoing {
			channel_id,
			content: String::new(),
			reply_to: None,
			ping_reply: true,
			files: Vec::new(),
			stickers: Vec::new(),
			poll: Some(poll),
		});
	}

	pub(super) fn post_forum(&mut self, post: ForumPost) {
		if post.name.trim().is_empty() || (post.content.trim().is_empty() && post.files.is_empty()) {
			self.reject("A post needs a title and a message");
			return;
		}
		let limit = self.state.read().upload_limit(post.channel_id);
		if let Some(reason) = too_large(&post.files, limit) {
			self.reject(reason);
			return;
		}
		self.remember_message_emoji(&post.content);
		let rest = self.rest.clone();
		let hooks = self.hooks.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			match create_post(&rest, &hooks, post).await {
				Ok(thread) => {
					let _ = internal.send(Internal::ThreadOpened(Box::new(thread)));
				}
				Err(error) => {
					tracing::warn!(%error, "forum post was not created");
					let _ = internal.send(Internal::Rejected(failure_reason("Create post", &error)));
				}
			}
		});
	}

	pub(super) fn dispatch_forward(
		&mut self,
		nonce: Nonce,
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
		message_id: Snowflake,
		to_channel_id: Snowflake,
	) {
		let body = CreateMessage {
			nonce: Some(nonce.0),
			message_reference: Some(MessageReference {
				kind: Some(MessageReferenceKind::Forward),
				message_id: Some(message_id),
				channel_id: Some(channel_id),
				guild_id,
				fail_if_not_exists: None,
			}),
			..CreateMessage::default()
		};
		self.typing_sent.remove(&to_channel_id);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			if let Err(error) = rest.create_message(to_channel_id, body).await {
				tracing::warn!(%error, "message was not forwarded");
				let _ = internal.send(Internal::Rejected(super::actions::failure_reason(
					"Forward", &error,
				)));
			}
		});
	}

	pub(super) fn send_failed(&mut self, nonce: Nonce, reason: SendFailure, outgoing: Outgoing) {
		self.outbox.keep(nonce.clone(), outgoing);
		self.apply_local(LocalEvent::PendingFailed { nonce, reason });
	}

	pub(super) fn retry_message(&mut self, channel_id: Snowflake, nonce: Nonce) {
		let outgoing = self.outbox.take(&nonce).or_else(|| {
			let state = self.state.read();
			let pending = state.messages(channel_id)?.pending.iter().find(|pending| {
				pending.failure.is_some() && pending.message.nonce.as_ref() == Some(&nonce)
			})?;
			Some(Outgoing {
				channel_id,
				content: pending.message.content.clone(),
				reply_to: None,
				ping_reply: true,
				files: Vec::new(),
				stickers: pending.message.sticker_items.to_vec(),
				poll: None,
			})
		});
		let Some(outgoing) = outgoing else {
			return;
		};
		self.apply_local(LocalEvent::PendingDiscarded { nonce });
		if let Some(poll) = outgoing.poll {
			self.on_command(crate::Command::SendPoll {
				channel_id: outgoing.channel_id,
				poll,
			});
			return;
		}
		if let Some(sticker) = outgoing.stickers.first() {
			self.on_command(crate::Command::SendSticker {
				channel_id: outgoing.channel_id,
				sticker_id: sticker.id,
			});
			return;
		}
		self.on_command(crate::Command::SendMessage {
			channel_id: outgoing.channel_id,
			content: outgoing.content,
			reply_to: outgoing.reply_to,
			ping_reply: outgoing.ping_reply,
			files: outgoing.files,
		});
	}
}

fn failed(pending: &PendingMessage) -> bool {
	pending.failure.is_some() || matches!(pending.command, Some(CommandStage::Failed(_)))
}

impl Core {
	pub(super) fn discard_message(&mut self, channel_id: Snowflake, nonce: Nonce) {
		let failed = self.state.read().messages(channel_id).is_some_and(|cache| {
			cache
				.pending
				.iter()
				.any(|pending| pending.message.nonce.as_ref() == Some(&nonce) && failed(pending))
		});
		self.outbox.take(&nonce);
		let cancelled = self.outbox.cancel(&nonce);
		if failed || cancelled {
			self.apply_local(LocalEvent::PendingDiscarded { nonce });
		}
	}
}
