use std::sync::Arc;

use protocol::{Message, MessageReferenceKind, MessageType, MessageUpdate, Permissions, Snowflake, User};

use crate::changes::Changes;
use crate::messages::apply_update;
use crate::state::{State, intern_user};

const MENTION_PREFIX: &str = "<@";

#[derive(Debug, Clone)]
pub enum Referenced {
	Found(Arc<Message>),
	Unavailable,
}

#[derive(Debug, Clone, Copy)]
pub enum ReplyTarget<'a> {
	Message(&'a Message),
	Deleted(&'a Message),
	Unavailable,
	Loading,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackfillNeeds {
	pub references: Vec<Snowflake>,
	pub users: Vec<Snowflake>,
	pub members: Vec<Snowflake>,
}

impl BackfillNeeds {
	pub fn is_empty(&self) -> bool {
		self.references.is_empty() && self.users.is_empty() && self.members.is_empty()
	}
}

#[derive(Debug, Clone)]
pub(crate) struct StoredReference {
	channel_id: Snowflake,
	referenced: Referenced,
}

pub fn reply_reference(message: &Message) -> Option<Snowflake> {
	if message.kind != MessageType::Reply {
		return None;
	}
	let reference = message.message_reference.as_ref()?;
	if !matches!(reference.kind, None | Some(MessageReferenceKind::Default)) {
		return None;
	}
	if reference
		.channel_id
		.is_some_and(|channel_id| channel_id != message.channel_id)
	{
		return None;
	}
	reference.message_id
}

fn push_unique(ids: &mut Vec<Snowflake>, id: Snowflake) {
	if !ids.contains(&id) {
		ids.push(id);
	}
}

fn mentioned_ids(message: &Message) -> Vec<Snowflake> {
	if !message.content.contains(MENTION_PREFIX) {
		return Vec::new();
	}
	markdown::parse(&message.content)
		.mentions()
		.users
		.into_iter()
		.map(Snowflake)
		.collect()
}

impl State {
	pub fn referenced_message(&self, message_id: Snowflake) -> Option<&Referenced> {
		self.referenced.get(&message_id).map(|stored| &stored.referenced)
	}

	pub fn reply_target<'a>(&'a self, message: &'a Message) -> Option<ReplyTarget<'a>> {
		if let Some(target) = reply_reference(message)
			&& let Some(kept) = self
				.messages
				.get(&message.channel_id)
				.and_then(|cache| cache.deleted_message(target))
		{
			return Some(if self.show_deleted {
				ReplyTarget::Deleted(kept)
			} else {
				ReplyTarget::Unavailable
			});
		}
		if let Some(embedded) = message.referenced_message.as_deref() {
			return Some(ReplyTarget::Message(embedded));
		}
		let target = reply_reference(message)?;
		if let Some(loaded) = self
			.messages
			.get(&message.channel_id)
			.and_then(|cache| cache.by_id.get(&target))
		{
			return Some(ReplyTarget::Message(loaded));
		}
		Some(match self.referenced_message(target) {
			Some(Referenced::Found(found)) => ReplyTarget::Message(found),
			Some(Referenced::Unavailable) => ReplyTarget::Unavailable,
			None => ReplyTarget::Loading,
		})
	}

	pub fn can_read_history(&self, channel_id: Snowflake) -> bool {
		self.my_permissions(channel_id)
			.contains(Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY)
	}

	pub fn backfill_needs(&self, channel_id: Snowflake, only: Option<Snowflake>) -> BackfillNeeds {
		let mut needs = BackfillNeeds::default();
		let Some(cache) = self.messages.get(&channel_id) else {
			return needs;
		};
		let guild_id = self.channel_guild(channel_id);
		let members = guild_id.and_then(|guild_id| self.guilds.get(&guild_id).map(|guild| &guild.members));
		let me = self.current_user_id();
		let readable = self.can_read_history(channel_id);
		let messages: Vec<&Arc<Message>> = match only {
			Some(id) => cache.by_id.get(&id).into_iter().collect(),
			None => cache.ordered.iter().collect(),
		};
		for message in messages {
			if readable
				&& matches!(self.reply_target(message), Some(ReplyTarget::Loading))
				&& let Some(target) = reply_reference(message)
			{
				push_unique(&mut needs.references, target);
			}
			for user_id in mentioned_ids(message) {
				let embedded = message.mentions.iter().any(|user| user.id == user_id);
				if !embedded && !self.users.contains_key(&user_id) {
					push_unique(&mut needs.users, user_id);
				}
			}
			let author = message.author.id;
			if let Some(members) = members
				&& message.webhook_id.is_none()
				&& message.member.is_none()
				&& Some(author) != me
				&& !members.contains_key(&author)
			{
				push_unique(&mut needs.members, author);
			}
		}
		needs
	}

	pub(crate) fn reference_fetched(
		&mut self,
		channel_id: Snowflake,
		message_id: Snowflake,
		found: Option<&Message>,
		changes: &mut Changes,
	) {
		let in_window = self
			.messages
			.get(&channel_id)
			.is_some_and(|cache| cache.by_id.contains_key(&message_id));
		if in_window {
			return;
		}
		let referenced = match found {
			Some(message) => {
				let guild_id = message.guild_id.or_else(|| self.channel_guild(channel_id));
				self.ingest_message_people(guild_id, message);
				Referenced::Found(Arc::new(message.clone()))
			}
			None => Referenced::Unavailable,
		};
		self.referenced.insert(
			message_id,
			StoredReference {
				channel_id,
				referenced,
			},
		);
		changes.messages.insert(channel_id);
	}

	pub(crate) fn user_fetched(&mut self, channel_id: Snowflake, user: &User, changes: &mut Changes) {
		intern_user(&mut self.users, user);
		changes.messages.insert(channel_id);
	}

	pub(crate) fn referenced_deleted(&mut self, ids: &[Snowflake], changes: &mut Changes) {
		for id in ids {
			if let Some(stored) = self.referenced.get(id) {
				let channel_id = stored.channel_id;
				self.referenced.insert(
					*id,
					StoredReference {
						channel_id,
						referenced: Referenced::Unavailable,
					},
				);
				changes.messages.insert(channel_id);
			}
		}
	}

	pub(crate) fn referenced_updated(&mut self, update: &MessageUpdate, changes: &mut Changes) {
		let Some(StoredReference {
			channel_id,
			referenced: Referenced::Found(found),
		}) = self.referenced.get(&update.id)
		else {
			return;
		};
		let channel_id = *channel_id;
		let mut message = (**found).clone();
		apply_update(&mut message, update);
		self.referenced.insert(
			update.id,
			StoredReference {
				channel_id,
				referenced: Referenced::Found(Arc::new(message)),
			},
		);
		changes.messages.insert(channel_id);
	}

	pub(crate) fn media_refreshed(&mut self, channel_id: Snowflake, fresh: &Message, changes: &mut Changes) {
		let refresh = |message: &mut Message| {
			message.attachments = fresh.attachments.clone();
			message.embeds = fresh.embeds.clone();
		};
		if let Some(cache) = self.messages.get_mut(&channel_id)
			&& cache.modify(fresh.id, refresh)
		{
			changes.messages.insert(channel_id);
		}
		if let Some(StoredReference {
			channel_id: stored_channel,
			referenced: Referenced::Found(found),
		}) = self.referenced.get(&fresh.id)
		{
			let stored_channel = *stored_channel;
			let mut message = (**found).clone();
			refresh(&mut message);
			self.referenced.insert(
				fresh.id,
				StoredReference {
					channel_id: stored_channel,
					referenced: Referenced::Found(Arc::new(message)),
				},
			);
			changes.messages.insert(stored_channel);
		}
	}

	pub fn holds_message(&self, channel_id: Snowflake, message_id: Snowflake) -> bool {
		self.messages
			.get(&channel_id)
			.is_some_and(|cache| cache.by_id.contains_key(&message_id))
			|| matches!(
				self.referenced.get(&message_id),
				Some(StoredReference {
					referenced: Referenced::Found(_),
					..
				})
			)
	}

	pub(crate) fn forget_references(&mut self, channel_id: Snowflake) {
		let stale: Vec<Snowflake> = self
			.referenced
			.iter()
			.filter(|(_, stored)| stored.channel_id == channel_id)
			.map(|(id, _)| *id)
			.collect();
		for id in stale {
			self.referenced.remove(&id);
		}
	}

	pub(crate) fn evict_references(&mut self) {
		self.referenced
			.evict_to(self.limits.referenced_messages, |_| false);
	}
}
