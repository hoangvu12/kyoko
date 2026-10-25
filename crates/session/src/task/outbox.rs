use std::collections::VecDeque;

use protocol::rest::{AllowedMentions, AttachmentDescriptor};
use protocol::{CreateMessage, Message, MessageReference, Nonce, Snowflake, StickerItem};
use rest::api::UploadSlotRequest;
use rest::{RestClient, RestError, UploadFile};
use state::{LocalEvent, MAX_ATTACHMENTS, SendFailure};

use crate::hooks::{Hooks, Rewrite, guarded_blocking};

use super::{Core, Internal, failures};

const KEPT_FAILURES: usize = 16;
const MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone)]
pub(crate) struct Outgoing {
	pub(crate) channel_id: Snowflake,
	pub(crate) content: String,
	pub(crate) reply_to: Option<Snowflake>,
	pub(crate) ping_reply: bool,
	pub(crate) files: Vec<UploadFile>,
	pub(crate) stickers: Vec<StickerItem>,
}

#[derive(Default)]
pub(crate) struct Outbox {
	failed: VecDeque<(Nonce, Outgoing)>,
}

impl Outbox {
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
		..CreateMessage::default()
	}
}

async fn upload_and_send(
	rest: &RestClient,
	mut body: CreateMessage,
	files: &[UploadFile],
	channel_id: Snowflake,
) -> Result<Message, RestError> {
	let requests: Vec<UploadSlotRequest> = files
		.iter()
		.enumerate()
		.map(|(index, file)| UploadSlotRequest {
			id: index.to_string(),
			filename: file.name.clone(),
			file_size: file.bytes.len() as u64,
			is_clip: false,
		})
		.collect();
	let slots = rest.attachment_slots(channel_id, &requests).await?;
	let mut attachments = Vec::with_capacity(files.len());
	for (index, file) in files.iter().enumerate() {
		let slot = slots
			.iter()
			.find(|slot| slot.id == index as i64)
			.ok_or(RestError::InvalidRequest)?;
		rest.upload(&slot.upload_url, file).await?;
		attachments.push(AttachmentDescriptor {
			id: Snowflake(index as u64),
			filename: Some(file.name.clone()),
			uploaded_filename: Some(slot.upload_filename.clone()),
			..AttachmentDescriptor::default()
		});
	}
	body.attachments = attachments;
	rest.create_message(channel_id, body).await
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
		if outgoing.content.trim().is_empty() && outgoing.files.is_empty() && outgoing.stickers.is_empty() {
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
			author: Box::new(author),
			by_plugin: None,
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
		self.runtime.spawn(async move {
			let channel_id = outgoing.channel_id;
			let sent = if outgoing.files.is_empty() {
				rest.create_message(channel_id, body).await
			} else {
				upload_and_send(&rest, body, &outgoing.files, channel_id).await
			};
			if let Err(error) = sent {
				tracing::warn!(%error, "message was not sent");
				let failure = failures::send_failure(&error);
				let _ = internal.send(Internal::SendFailed(nonce, failure, Box::new(outgoing)));
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
			})
		});
		let Some(outgoing) = outgoing else {
			return;
		};
		self.apply_local(LocalEvent::PendingDiscarded { nonce });
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

	pub(super) fn discard_message(&mut self, channel_id: Snowflake, nonce: Nonce) {
		let failed = self.state.read().messages(channel_id).is_some_and(|cache| {
			cache
				.pending
				.iter()
				.any(|pending| pending.failure.is_some() && pending.message.nonce.as_ref() == Some(&nonce))
		});
		self.outbox.take(&nonce);
		if failed {
			self.apply_local(LocalEvent::PendingDiscarded { nonce });
		}
	}
}
