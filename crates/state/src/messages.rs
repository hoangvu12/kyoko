use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use protocol::{Message, MessageUpdate, Nonce, PartialEmoji, Reaction, Snowflake};

use crate::caps::CacheLimits;
use crate::event::LoadDirection;
use crate::views::RenderedMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
	pub first: Snowflake,
	pub last: Snowflake,
	pub at_bottom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendFailure {
	RateLimited { retry_after: Duration },
	Forbidden,
	MissingPermissions,
	Captcha { service: Option<String> },
	Unauthorized,
	Network,
	Other(Option<u16>),
}

#[derive(Debug, Clone)]
pub struct PendingMessage {
	pub message: Arc<Message>,
	pub failure: Option<SendFailure>,
	pub plugin: Option<String>,
	pub altered_by: Option<String>,
	pub rendered: Option<Arc<RenderedMessage>>,
}

#[derive(Debug, Default)]
pub struct ChannelMessages {
	pub ordered: Vec<Arc<Message>>,
	pub by_id: HashMap<Snowflake, Arc<Message>>,
	pub rendered: HashMap<Snowflake, Arc<RenderedMessage>>,
	pub reached_start: bool,
	pub pending: Vec<PendingMessage>,
	pub gaps: Vec<(Snowflake, Snowflake)>,
	detached: bool,
	window: usize,
	viewport: Option<Viewport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
	Oldest,
	Newest,
}

impl ChannelMessages {
	pub fn newest_id(&self) -> Option<Snowflake> {
		self.ordered.last().map(|message| message.id)
	}

	pub fn oldest_id(&self) -> Option<Snowflake> {
		self.ordered.first().map(|message| message.id)
	}

	pub fn resident(&self) -> Option<(Snowflake, Snowflake)> {
		Some((self.oldest_id()?, self.newest_id()?))
	}

	pub fn is_resident(&self, first: Snowflake, last: Snowflake) -> bool {
		self.resident()
			.is_some_and(|(oldest, newest)| oldest <= first && last <= newest)
	}

	pub fn viewport(&self) -> Option<Viewport> {
		self.viewport
	}

	pub fn window(&self, limits: &CacheLimits) -> usize {
		self.window.clamp(limits.window(), limits.ceiling())
	}

	pub fn is_detached(&self) -> bool {
		self.detached
	}

	pub fn push_live(&mut self, message: Arc<Message>, limits: &CacheLimits) -> bool {
		if self.detached {
			self.extend_tail_gap(message.id);
			return false;
		}
		let id = message.id;
		self.insert(message);
		let pinned = self.ordered.len() - self.room(End::Oldest);
		self.window = self.window(limits).max(pinned.min(limits.ceiling()));
		self.trim(self.window, End::Oldest);
		self.by_id.contains_key(&id)
	}

	pub fn merge_loaded(
		&mut self,
		messages: Vec<Arc<Message>>,
		direction: LoadDirection,
		reached_start: bool,
		channel_last: Option<Snowflake>,
		open: bool,
		limits: &CacheLimits,
	) {
		let was_empty = self.ordered.is_empty();
		let received = messages.len();
		if direction == LoadDirection::Around {
			self.ordered.clear();
			self.by_id.clear();
			self.rendered.clear();
			self.gaps.clear();
			self.window = 0;
		}
		for message in messages {
			self.insert(message);
		}
		let widened = if open && !was_empty {
			self.ordered.len().clamp(limits.window(), limits.ceiling())
		} else {
			self.window(limits)
		};
		match direction {
			LoadDirection::History => {
				self.reached_start = reached_start;
				if was_empty {
					self.detached = false;
				}
				if open && !was_empty {
					self.window = widened;
					self.trim(widened, End::Newest);
				} else {
					self.trim(widened, End::Oldest);
				}
			}
			LoadDirection::Around => {
				self.reached_start = reached_start;
				self.detached = self.behind(channel_last);
				let end = if self.detached { End::Newest } else { End::Oldest };
				self.trim(self.window(limits), end);
			}
			LoadDirection::Newer => {
				self.detached = received != 0 && self.behind(channel_last);
				self.window = widened;
				self.trim(widened, End::Oldest);
			}
		}
		self.reset_tail_gap(channel_last);
	}

	pub fn set_viewport(&mut self, viewport: Viewport, limits: &CacheLimits) -> bool {
		self.viewport = Some(viewport);
		if viewport.at_bottom && !self.detached {
			return self.shrink(limits);
		}
		false
	}

	pub fn leave(&mut self, limits: &CacheLimits) -> bool {
		self.viewport = None;
		self.shrink(limits)
	}

	fn shrink(&mut self, limits: &CacheLimits) -> bool {
		self.window = 0;
		let before = self.ordered.len();
		self.trim(limits.window(), End::Oldest);
		self.ordered.len() != before
	}

	fn protected_span(&self) -> Option<(usize, usize)> {
		let viewport = self.viewport?;
		let start = self
			.ordered
			.partition_point(|message| message.id < viewport.first);
		let end = self
			.ordered
			.partition_point(|message| message.id <= viewport.last);
		(start < end).then_some((start, end))
	}

	fn room(&self, end: End) -> usize {
		let len = self.ordered.len();
		match (self.protected_span(), end) {
			(None, _) => len,
			(Some((start, _)), End::Oldest) => start,
			(Some((_, stop)), End::Newest) => len - stop,
		}
	}

	fn trim(&mut self, limit: usize, prefer: End) {
		let other = match prefer {
			End::Oldest => End::Newest,
			End::Newest => End::Oldest,
		};
		for end in [prefer, other] {
			let excess = self.ordered.len().saturating_sub(limit);
			if excess == 0 {
				return;
			}
			let count = excess.min(self.room(end));
			match end {
				End::Oldest => self.drop_oldest(count),
				End::Newest => self.drop_newest(count),
			}
		}
	}

	fn drop_oldest(&mut self, count: usize) {
		if count == 0 {
			return;
		}
		for message in self.ordered.drain(..count) {
			self.by_id.remove(&message.id);
			self.rendered.remove(&message.id);
		}
		self.reached_start = false;
	}

	fn drop_newest(&mut self, count: usize) {
		if count == 0 {
			return;
		}
		let keep = self.ordered.len() - count;
		let mut newest_dropped = Snowflake::default();
		for message in self.ordered.drain(keep..) {
			self.by_id.remove(&message.id);
			self.rendered.remove(&message.id);
			newest_dropped = newest_dropped.max(message.id);
		}
		self.detached = true;
		self.extend_tail_gap(newest_dropped);
	}

	pub fn remove(&mut self, id: Snowflake) -> bool {
		let Ok(index) = self.ordered.binary_search_by_key(&id, |message| message.id) else {
			return false;
		};
		self.ordered.remove(index);
		self.by_id.remove(&id);
		self.rendered.remove(&id);
		true
	}

	pub fn modify(&mut self, id: Snowflake, edit: impl FnOnce(&mut Message)) -> bool {
		let Ok(index) = self.ordered.binary_search_by_key(&id, |message| message.id) else {
			return false;
		};
		let mut message = (*self.ordered[index]).clone();
		edit(&mut message);
		let message = Arc::new(message);
		self.by_id.insert(id, Arc::clone(&message));
		self.ordered[index] = message;
		true
	}

	pub fn add_pending(
		&mut self,
		message: Arc<Message>,
		plugin: Option<String>,
		rendered: Arc<RenderedMessage>,
	) {
		self.pending.push(PendingMessage {
			message,
			failure: None,
			plugin,
			altered_by: None,
			rendered: Some(rendered),
		});
	}

	pub fn take_pending(&mut self, nonce: &Nonce) -> Option<PendingMessage> {
		let index = self
			.pending
			.iter()
			.position(|pending| pending.message.nonce.as_ref() == Some(nonce))?;
		Some(self.pending.remove(index))
	}

	pub fn remove_pending(&mut self, nonce: &Nonce) -> bool {
		self.take_pending(nonce).is_some()
	}

	pub fn fail_pending(&mut self, nonce: &Nonce, failure: SendFailure) -> bool {
		match self.pending_mut(nonce) {
			Some(pending) => {
				pending.failure = Some(failure);
				true
			}
			None => false,
		}
	}

	pub fn alter_pending(&mut self, nonce: &Nonce, content: String, by: String) -> bool {
		let Some(pending) = self.pending_mut(nonce) else {
			return false;
		};
		let mut message = (*pending.message).clone();
		pending.rendered = Some(Arc::new(RenderedMessage::plain(&content, false, 0)));
		message.content = content;
		pending.message = Arc::new(message);
		pending.altered_by = Some(by);
		true
	}

	fn pending_mut(&mut self, nonce: &Nonce) -> Option<&mut PendingMessage> {
		self.pending
			.iter_mut()
			.find(|pending| pending.message.nonce.as_ref() == Some(nonce))
	}

	fn insert(&mut self, message: Arc<Message>) {
		self.by_id.insert(message.id, Arc::clone(&message));
		match self
			.ordered
			.binary_search_by_key(&message.id, |existing| existing.id)
		{
			Ok(index) => self.ordered[index] = message,
			Err(index) => self.ordered.insert(index, message),
		}
	}

	fn behind(&self, channel_last: Option<Snowflake>) -> bool {
		match (self.newest_id(), channel_last) {
			(Some(newest), Some(last)) => newest < last,
			_ => false,
		}
	}

	fn extend_tail_gap(&mut self, id: Snowflake) {
		let newest = self.newest_id().unwrap_or_default();
		match self.gaps.last_mut() {
			Some(gap) if gap.0 == newest => gap.1 = gap.1.max(id),
			_ => self.gaps.push((newest, id)),
		}
	}

	fn reset_tail_gap(&mut self, upper: Option<Snowflake>) {
		let previous = self.gaps.last().map(|gap| gap.1);
		self.gaps.clear();
		if !self.detached {
			return;
		}
		let newest = self.newest_id().unwrap_or_default();
		let upper = upper.max(previous).unwrap_or(newest).max(newest);
		self.gaps.push((newest, upper));
	}
}

pub fn apply_update(message: &mut Message, update: &MessageUpdate) {
	if let Some(author) = &update.author {
		message.author = author.clone();
	}
	if let Some(member) = &update.member {
		message.member = Some(member.clone());
	}
	if let Some(content) = &update.content {
		message.content.clone_from(content);
	}
	if update.edited_timestamp.is_some() {
		message.edited_timestamp = update.edited_timestamp;
	}
	if let Some(tts) = update.tts {
		message.tts = tts;
	}
	if let Some(everyone) = update.mention_everyone {
		message.mention_everyone = everyone;
	}
	if let Some(mentions) = &update.mentions {
		message.mentions = mentions.clone();
	}
	if let Some(roles) = &update.mention_roles {
		message.mention_roles.clone_from(roles);
	}
	if let Some(channels) = &update.mention_channels {
		message.mention_channels = channels.clone();
	}
	if let Some(attachments) = &update.attachments {
		message.attachments = attachments.clone();
	}
	if let Some(embeds) = &update.embeds {
		message.embeds = embeds.clone();
	}
	if let Some(reactions) = &update.reactions {
		message.reactions = reactions.clone();
	}
	if let Some(pinned) = update.pinned {
		message.pinned = pinned;
	}
	if let Some(kind) = update.kind {
		message.kind = kind;
	}
	if let Some(flags) = update.flags {
		message.flags = flags;
	}
	if let Some(reference) = &update.message_reference {
		message.message_reference = Some(reference.clone());
	}
	if let Some(referenced) = &update.referenced_message {
		message.referenced_message = Some(referenced.clone());
	}
	if let Some(components) = &update.components {
		message.components.clone_from(components);
	}
	if let Some(stickers) = &update.sticker_items {
		message.sticker_items = stickers.clone();
	}
	if let Some(poll) = &update.poll {
		message.poll = Some(poll.clone());
	}
}

fn same_emoji(a: &PartialEmoji, b: &PartialEmoji) -> bool {
	a.id == b.id && (a.id.is_some() || a.name == b.name)
}

pub fn add_reaction(message: &mut Message, emoji: &PartialEmoji, mine: bool) {
	match message
		.reactions
		.iter_mut()
		.find(|reaction| same_emoji(&reaction.emoji, emoji))
	{
		Some(reaction) => {
			reaction.count += 1;
			reaction.me |= mine;
		}
		None => message.reactions.push(Reaction {
			count: 1,
			me: mine,
			emoji: emoji.clone(),
			..Reaction::default()
		}),
	}
}

pub fn remove_reaction(message: &mut Message, emoji: &PartialEmoji, mine: bool) {
	if let Some(reaction) = message
		.reactions
		.iter_mut()
		.find(|reaction| same_emoji(&reaction.emoji, emoji))
	{
		reaction.count -= 1;
		if mine {
			reaction.me = false;
		}
	}
	message.reactions.retain(|reaction| reaction.count > 0);
}

pub fn remove_emoji(message: &mut Message, emoji: &PartialEmoji) {
	message
		.reactions
		.retain(|reaction| !same_emoji(&reaction.emoji, emoji));
}
