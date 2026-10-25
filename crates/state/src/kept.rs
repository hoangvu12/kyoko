use std::sync::Arc;

use chrono::{DateTime, Utc};
use protocol::{Attachment, Embed, Message, MessageUpdate, Snowflake};
use serde::{Deserialize, Serialize};

use crate::changes::Changes;
use crate::references::Referenced;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeletedInfo {
	pub at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeletedMessage {
	pub(crate) message: Arc<Message>,
	pub(crate) info: DeletedInfo,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageVersion {
	pub content: String,
	pub edited_timestamp: Option<DateTime<Utc>>,
	pub replaced_at: Option<DateTime<Utc>>,
	pub attachments: Vec<Attachment>,
	pub embeds: Vec<Embed>,
}

#[derive(Debug, Clone)]
pub enum KeptWrite {
	Deleted {
		channel_id: Snowflake,
		message_id: Snowflake,
		message: Option<Arc<Message>>,
		at: DateTime<Utc>,
	},
	Forgotten {
		message_id: Snowflake,
	},
	Version {
		channel_id: Snowflake,
		message_id: Snowflake,
		version: MessageVersion,
	},
}

pub fn replaced_version(message: &Message, update: &MessageUpdate) -> Option<MessageVersion> {
	let edited = update.edited_timestamp?;
	let retimed = message.edited_timestamp != Some(edited);
	let content = update
		.content
		.as_ref()
		.is_some_and(|content| *content != message.content);
	let attachments = update
		.attachments
		.as_ref()
		.is_some_and(|attachments| attachments[..] != message.attachments[..]);
	let embeds = update
		.embeds
		.as_ref()
		.is_some_and(|embeds| embeds[..] != message.embeds[..]);
	(content || (retimed && (attachments || embeds))).then(|| MessageVersion {
		content: message.content.clone(),
		edited_timestamp: message.edited_timestamp,
		replaced_at: Some(edited),
		attachments: message.attachments.to_vec(),
		embeds: message.embeds.to_vec(),
	})
}

impl State {
	pub fn shows_deleted_messages(&self) -> bool {
		self.show_deleted
	}

	pub fn deleted(&self, channel_id: Snowflake, message_id: Snowflake) -> Option<DeletedInfo> {
		self.messages.get(&channel_id)?.deleted(message_id)
	}

	pub fn edit_history(&self, channel_id: Snowflake, message_id: Snowflake) -> &[MessageVersion] {
		self.messages
			.get(&channel_id)
			.map_or(&[], |cache| cache.edit_history(message_id))
	}

	pub fn deleted_by_me(&self, message_id: Snowflake) -> bool {
		self.deletions.iter().any(|batch| {
			matches!(
				batch.status(message_id),
				Some(crate::deletions::DeleteStatus::Queued | crate::deletions::DeleteStatus::Deleted)
			)
		})
	}

	pub fn take_kept_writes(&mut self) -> Vec<KeptWrite> {
		std::mem::take(&mut self.kept_writes)
	}

	pub(crate) fn show_deleted_changed(&mut self, show: bool, changes: &mut Changes) {
		if self.show_deleted != show {
			self.show_deleted = show;
			changes.messages.mark_all();
		}
	}

	pub(crate) fn keep_deleted(&mut self, channel_id: Snowflake, ids: &[Snowflake], changes: &mut Changes) {
		let at = Utc::now();
		let limits = self.limits;
		for &message_id in ids {
			if self.deleted_by_me(message_id) {
				if let Some(cache) = self.messages.get_mut(&channel_id)
					&& cache.remove(message_id)
				{
					changes.messages.insert(channel_id);
				}
				self.kept_writes.push(KeptWrite::Forgotten { message_id });
				continue;
			}
			let referenced = match self.referenced_message(message_id) {
				Some(Referenced::Found(found)) => Some(Arc::clone(found)),
				_ => None,
			};
			let mut message = None;
			if let Some(cache) = self.messages.get_mut(&channel_id) {
				let info = DeletedInfo { at };
				if cache.mark_deleted(message_id, info) {
					changes.messages.insert(channel_id);
				} else if let Some(found) = referenced {
					cache.restore_deleted(Arc::clone(&found), info);
				}
				message = cache.deleted_message(message_id).cloned();
			}
			self.kept_writes.push(KeptWrite::Deleted {
				channel_id,
				message_id,
				message,
				at,
			});
		}
		if let Some(cache) = self.messages.get_mut(&channel_id)
			&& cache.cap_kept(&limits)
		{
			changes.messages.insert(channel_id);
		}
	}

	pub(crate) fn keep_version(&mut self, update: &MessageUpdate, changes: &mut Changes) {
		let limits = self.limits;
		let Some(cache) = self.messages.get_mut(&update.channel_id) else {
			return;
		};
		if cache.deleted(update.id).is_some() {
			return;
		}
		let Some(version) = cache
			.by_id
			.get(&update.id)
			.and_then(|message| replaced_version(message, update))
		else {
			return;
		};
		cache.push_version(update.id, version.clone());
		cache.cap_kept(&limits);
		changes.messages.insert(update.channel_id);
		self.kept_writes.push(KeptWrite::Version {
			channel_id: update.channel_id,
			message_id: update.id,
			version,
		});
	}

	pub(crate) fn restore_kept(
		&mut self,
		channel_id: Snowflake,
		deleted: &[(Message, DateTime<Utc>)],
		versions: &[(Snowflake, Vec<MessageVersion>)],
		changes: &mut Changes,
	) {
		self.warm_up(channel_id, changes);
		let limits = self.limits;
		let cache = self.messages.entry(channel_id).or_default();
		for (message, at) in deleted {
			cache.restore_deleted(Arc::new(message.clone()), DeletedInfo { at: *at });
		}
		for (message_id, history) in versions {
			cache.restore_versions(*message_id, history);
		}
		let spliced = cache.splice_deleted();
		cache.cap_kept(&limits);
		self.unrendered.extend(spliced);
		changes.messages.insert(channel_id);
	}
}
