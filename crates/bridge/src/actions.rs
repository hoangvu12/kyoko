use chrono::{DateTime, Utc};
use protocol::{ChannelType, MessageType, Permissions, Snowflake};
use state::{DeleteStatus, RiskyLock, SendFailure, State};

use crate::message::reaction_key;
use crate::timeline::MessageEntry;

pub const REPORT_URL: &str = "https://support.discord.com/hc/en-us/requests/new";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Access {
	pub guild_id: Option<Snowflake>,
	pub channel_id: Option<Snowflake>,
	pub send: bool,
	pub read_history: bool,
	pub add_reactions: bool,
	pub manage_messages: bool,
	pub create_threads: bool,
	pub in_thread: bool,
	pub lock: Option<RiskyLock>,
}

impl Access {
	pub fn of(state: &State, guild_id: Option<Snowflake>, channel_id: Snowflake, now: DateTime<Utc>) -> Self {
		let permissions = state.my_permissions(channel_id);
		let guild = guild_id.is_some();
		let has = |needed: Permissions| permissions.contains(Permissions::VIEW_CHANNEL | needed);
		Self {
			guild_id,
			channel_id: Some(channel_id),
			send: state.can_send(channel_id),
			read_history: has(Permissions::READ_MESSAGE_HISTORY),
			add_reactions: has(Permissions::ADD_REACTIONS),
			manage_messages: guild && has(Permissions::MANAGE_MESSAGES),
			create_threads: guild && has(Permissions::CREATE_PUBLIC_THREADS),
			in_thread: state.channel(channel_id).is_some_and(|channel| {
				matches!(
					channel.kind,
					ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::AnnouncementThread
				)
			}),
			lock: state.channel_lock(channel_id, now),
		}
	}

	pub fn can_pin(&self) -> bool {
		self.manage_messages || self.guild_id.is_none()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageAction {
	React,
	ViewReactions,
	Reply,
	Edit,
	CopyText,
	CopyLink,
	Forward,
	CopyId,
	Pin,
	Unpin,
	CreateThread,
	MarkUnread,
	Select,
	Deselect,
	DeleteSelected,
	Delete,
	Report,
	RemoveEmbeds,
}

impl MessageAction {
	pub fn label(self, selected: usize) -> String {
		match self {
			Self::React => "Add Reaction".to_owned(),
			Self::ViewReactions => "View Reactions".to_owned(),
			Self::Reply => "Reply".to_owned(),
			Self::Edit => "Edit Message".to_owned(),
			Self::CopyText => "Copy Text".to_owned(),
			Self::CopyLink => "Copy Message Link".to_owned(),
			Self::Forward => "Forward".to_owned(),
			Self::CopyId => "Copy Message ID".to_owned(),
			Self::Pin => "Pin Message".to_owned(),
			Self::Unpin => "Unpin Message".to_owned(),
			Self::CreateThread => "Create Thread".to_owned(),
			Self::MarkUnread => "Mark Unread".to_owned(),
			Self::Select => "Select Message".to_owned(),
			Self::Deselect => "Deselect Message".to_owned(),
			Self::DeleteSelected => format!("Delete {selected} Selected"),
			Self::Delete => "Delete Message".to_owned(),
			Self::Report => "Report Message".to_owned(),
			Self::RemoveEmbeds => "Remove all embeds".to_owned(),
		}
	}

	pub fn icon(self) -> &'static str {
		match self {
			Self::React => "emoji",
			Self::ViewReactions => "add_reaction",
			Self::Reply => "reply",
			Self::Edit => "edit",
			Self::CopyText | Self::CopyId => "copy",
			Self::CopyLink => "link",
			Self::Forward => "forward",
			Self::Pin => "pin",
			Self::Unpin => "unpin",
			Self::CreateThread => "new_thread",
			Self::MarkUnread => "inbox",
			Self::Select | Self::Deselect => "check",
			Self::DeleteSelected | Self::Delete => "trash",
			Self::Report => "flag",
			Self::RemoveEmbeds => "close",
		}
	}

	pub fn is_danger(self) -> bool {
		matches!(self, Self::DeleteSelected | Self::Delete | Self::Report)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
	Action(MessageAction),
	Plugin(usize),
	Separator,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuContext {
	pub developer_mode: bool,
	pub selected: bool,
	pub selection: usize,
}

pub fn has_actions(entry: &MessageEntry) -> bool {
	entry.is_plain() && !entry.pending && !entry.ephemeral
}

pub fn can_delete(entry: &MessageEntry, access: &Access) -> bool {
	has_actions(entry) && !entry.deleted && (entry.mine || access.manage_messages)
}

pub fn can_edit(entry: &MessageEntry) -> bool {
	has_actions(entry) && !entry.deleted && entry.mine
}

pub fn can_remove_embeds(entry: &MessageEntry) -> bool {
	can_edit(entry)
		&& !entry.message.embeds.is_empty()
		&& !entry
			.message
			.flags
			.contains(protocol::MessageFlags::SUPPRESS_EMBEDS)
}

pub fn can_reply(entry: &MessageEntry, access: &Access) -> bool {
	has_actions(entry) && !entry.deleted && access.send
}

pub fn can_add_reaction(entry: &MessageEntry, access: &Access) -> bool {
	has_actions(entry) && !entry.deleted && access.read_history && access.add_reactions
}

pub fn can_join_reaction(entry: &MessageEntry, access: &Access) -> bool {
	has_actions(entry) && !entry.deleted && access.read_history
}

pub fn can_add_super_reaction(entry: &MessageEntry, access: &Access, nitro: bool) -> bool {
	nitro && can_add_reaction(entry, access)
}

fn threadable(entry: &MessageEntry) -> bool {
	matches!(entry.kind, MessageType::Default | MessageType::Reply) && entry.message.thread.is_none()
}

#[derive(Default)]
struct Menu(Vec<MenuEntry>);

impl Menu {
	fn add(&mut self, action: MessageAction) {
		self.0.push(MenuEntry::Action(action));
	}

	fn add_if(&mut self, shown: bool, action: MessageAction) {
		if shown {
			self.add(action);
		}
	}

	fn separator(&mut self) {
		if matches!(self.0.last(), Some(MenuEntry::Action(_) | MenuEntry::Plugin(_))) {
			self.0.push(MenuEntry::Separator);
		}
	}
}

pub fn message_menu(entry: &MessageEntry, access: &Access, context: MenuContext) -> Vec<MenuEntry> {
	use MessageAction::*;
	let mut menu = Menu::default();
	if !has_actions(entry) {
		return menu.0;
	}
	if entry.deleted {
		menu.add(CopyText);
		menu.add_if(context.developer_mode, CopyId);
		return menu.0;
	}
	let deletable = can_delete(entry, access);
	menu.add_if(can_add_reaction(entry, access), React);
	menu.add_if(!entry.message.reactions.is_empty(), ViewReactions);
	menu.separator();
	menu.add_if(can_edit(entry), Edit);
	menu.add_if(can_reply(entry, access), Reply);
	menu.add(Forward);
	menu.add_if(
		access.create_threads && !access.in_thread && threadable(entry),
		CreateThread,
	);
	menu.separator();
	menu.add(CopyText);
	if access.can_pin() {
		menu.add(if entry.message.pinned { Unpin } else { Pin });
	}
	menu.add(MarkUnread);
	menu.add(CopyLink);
	menu.separator();
	if deletable {
		menu.add(if context.selected { Deselect } else { Select });
	}
	menu.add_if(context.selection > 0, DeleteSelected);
	menu.add_if(deletable, Delete);
	menu.add_if(!entry.mine, Report);
	if context.developer_mode {
		menu.separator();
		menu.add(CopyId);
	}
	menu.0
}

pub fn toolbar_actions(entry: &MessageEntry, access: &Access) -> Vec<MessageAction> {
	let mut actions = Vec::new();
	if can_remove_embeds(entry) {
		actions.push(MessageAction::RemoveEmbeds);
	}
	if can_add_reaction(entry, access) {
		actions.push(MessageAction::React);
	}
	if can_reply(entry, access) {
		actions.push(MessageAction::Reply);
	}
	if can_edit(entry) {
		actions.push(MessageAction::Edit);
	}
	actions
}

pub fn message_link(guild_id: Option<Snowflake>, channel_id: Snowflake, message_id: Snowflake) -> String {
	let scope = guild_id.map_or_else(|| "@me".to_owned(), |guild_id| guild_id.to_string());
	format!("https://discord.com/channels/{scope}/{channel_id}/{message_id}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReactionToggle {
	Add(String),
	Remove(String),
}

pub fn reaction_toggle(entry: &MessageEntry, emoji: &str) -> ReactionToggle {
	reaction_toggle_of(entry, emoji, false)
}

pub fn reaction_toggle_of(entry: &MessageEntry, emoji: &str, burst: bool) -> ReactionToggle {
	let mine = entry.message.reactions.iter().any(|reaction| {
		let reacted = if burst { reaction.me_burst } else { reaction.me };
		reacted && reaction_key(&reaction.emoji) == emoji
	});
	if mine {
		ReactionToggle::Remove(emoji.to_owned())
	} else {
		ReactionToggle::Add(emoji.to_owned())
	}
}

pub fn reaction_allowed(entry: &MessageEntry, access: &Access, toggle: &ReactionToggle) -> bool {
	match toggle {
		ReactionToggle::Remove(_) => has_actions(entry) && !entry.deleted,
		ReactionToggle::Add(emoji) => {
			let existing = entry
				.message
				.reactions
				.iter()
				.any(|reaction| reaction_key(&reaction.emoji) == *emoji);
			if existing {
				can_join_reaction(entry, access)
			} else {
				can_add_reaction(entry, access)
			}
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteProgress {
	pub batch: u64,
	pub total: usize,
	pub deleted: usize,
	pub refused: usize,
	pub cancelled: usize,
	pub failure: Option<SendFailure>,
}

impl DeleteProgress {
	pub fn of(state: &State, channel_id: Snowflake) -> Option<Self> {
		let batch = state.deletions().next_back()?;
		if batch.channel_id != channel_id {
			return None;
		}
		let mut progress = Self {
			batch: batch.id,
			total: batch.messages.len(),
			deleted: 0,
			refused: 0,
			cancelled: 0,
			failure: None,
		};
		for (_, status) in &batch.messages {
			match status {
				DeleteStatus::Queued => {}
				DeleteStatus::Deleted => progress.deleted += 1,
				DeleteStatus::Refused => progress.refused += 1,
				DeleteStatus::Cancelled => progress.cancelled += 1,
				DeleteStatus::Failed(failure) => progress.failure = Some(failure.clone()),
			}
		}
		Some(progress)
	}

	pub fn finished(&self) -> bool {
		self.deleted + self.refused + self.cancelled + usize::from(self.failure.is_some()) >= self.total
	}

	pub fn summary(&self) -> String {
		if !self.finished() {
			return format!("Deleting {} of {}\u{2026}", self.deleted + 1, self.total);
		}
		let mut parts = vec![format!("Deleted {} of {}", self.deleted, self.total)];
		if self.refused > 0 {
			parts.push(format!("{} not yours to delete", self.refused));
		}
		if let Some(failure) = &self.failure {
			parts.push(delete_failure(failure));
		}
		if self.cancelled > 0 {
			parts.push(format!("{} not attempted", self.cancelled));
		}
		parts.join(", ")
	}
}

fn delete_failure(failure: &SendFailure) -> String {
	match failure {
		SendFailure::RateLimited { .. } => "rate limited, try again shortly".to_owned(),
		SendFailure::Forbidden | SendFailure::MissingPermissions => "missing permissions".to_owned(),
		SendFailure::Captcha { .. } => "complete this in the official Discord client".to_owned(),
		SendFailure::Unauthorized => "signed out".to_owned(),
		SendFailure::Network => "network error".to_owned(),
		SendFailure::Other(Some(status)) => format!("failed with status {status}"),
		SendFailure::Other(None) => "failed".to_owned(),
	}
}
