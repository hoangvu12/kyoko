use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use markdown::Document;
use protocol::{Message, MessageType, Snowflake, User};
use session::{Badge, Command, DeletedLook, Formatting, MessageLook, ShownName};
use state::{
	Changes, CommandStage, DEFERRED_REPLY_WINDOW, MessageRow, MessageVersion, MessageView, RenderedMessage,
	State, UploadProgress,
};

use crate::display::{Display, person};
use crate::message::{Body, Names, ReplyPreview, SystemLine, person_name, system_line};
use crate::sidebar::ChannelKind;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowKey {
	Loading,
	Start,
	Day(NaiveDate),
	Unread,
	Message(Snowflake),
	Pending(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
	Loading,
	Start,
	Day(NaiveDate),
	Unread,
	Message(Box<MessageEntry>),
}

#[derive(Debug, Clone)]
pub struct MessageEntry {
	pub id: Snowflake,
	pub author: String,
	pub author_extra: Option<String>,
	pub author_color: Option<u32>,
	pub decorations: Vec<Badge>,
	pub content: String,
	pub sent_at: DateTime<Utc>,
	pub kind: MessageType,
	pub grouped: bool,
	pub edited: bool,
	pub pending: bool,
	pub failed: bool,
	pub upload: Option<UploadProgress>,
	pub deleted: bool,
	pub bot: bool,
	pub mine: bool,
	pub highlight: Highlight,
	pub message: Arc<Message>,
	pub rendered: Option<Arc<RenderedMessage>>,
	pub body: Arc<Body>,
	pub names: Arc<Names>,
	pub reply: Option<ReplyPreview>,
	pub system: Option<SystemLine>,
	pub look: Option<MessageLook>,
	pub history: Arc<[Document]>,
	pub interaction: Option<InteractionLine>,
	pub ephemeral: bool,
	pub reply_state: Option<ReplyState>,
	pub interaction_failed: bool,
	pub waiting_component: Option<String>,
	pub select_options: Arc<crate::components::SelectOptions>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionLine {
	pub user: User,
	pub name: String,
	pub command: String,
	pub color: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyState {
	Sending,
	Thinking,
	Failed(String),
	TimedOut,
}

impl ReplyState {
	pub fn failure(&self) -> Option<&str> {
		match self {
			ReplyState::Failed(reason) => Some(reason),
			ReplyState::TimedOut => Some(NO_RESPONSE),
			ReplyState::Sending | ReplyState::Thinking => None,
		}
	}
}

pub const NO_RESPONSE: &str = "The application did not respond";

fn reply_state(view: &MessageView, now: DateTime<Utc>) -> Option<ReplyState> {
	match &view.command {
		Some(CommandStage::Sending) => Some(ReplyState::Sending),
		Some(CommandStage::Thinking) => Some(ReplyState::Thinking),
		Some(CommandStage::Failed(reason)) if reason.is_empty() => Some(ReplyState::TimedOut),
		Some(CommandStage::Failed(reason)) => Some(ReplyState::Failed(reason.clone())),
		None if view.message.is_loading() => {
			let deadline = view.message.id.timestamp() + DEFERRED_REPLY_WINDOW;
			if view.expired || now >= deadline {
				Some(ReplyState::TimedOut)
			} else {
				Some(ReplyState::Thinking)
			}
		}
		None => None,
	}
}

fn interaction_line(
	state: &State,
	guild_id: Option<Snowflake>,
	message: &Message,
) -> Option<InteractionLine> {
	let header = message.interaction_header()?;
	Some(InteractionLine {
		name: person_name(state, guild_id, &header.user),
		color: guild_id.and_then(|guild_id| state.role_color(guild_id, header.user.id)),
		user: header.user,
		command: header.name,
	})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Highlight {
	#[default]
	None,
	Mention,
	ReplyToYou,
}

fn highlight(state: &State, guild_id: Option<Snowflake>, message: &Message) -> Highlight {
	let me = state.current_user_id();
	if me.is_none() || Some(message.author.id) == me {
		return Highlight::None;
	}
	if state.mentions_me(message, guild_id) {
		return Highlight::Mention;
	}
	let replied_to_me = message
		.referenced_message
		.as_ref()
		.is_some_and(|original| Some(original.author.id) == me);
	if replied_to_me {
		Highlight::ReplyToYou
	} else {
		Highlight::None
	}
}

impl PartialEq for MessageEntry {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
			&& self.author == other.author
			&& self.author_extra == other.author_extra
			&& self.author_color == other.author_color
			&& self.decorations == other.decorations
			&& self.content == other.content
			&& self.sent_at == other.sent_at
			&& self.kind == other.kind
			&& self.grouped == other.grouped
			&& self.edited == other.edited
			&& self.pending == other.pending
			&& self.failed == other.failed
			&& self.upload == other.upload
			&& self.deleted == other.deleted
			&& self.bot == other.bot
			&& self.mine == other.mine
			&& self.highlight == other.highlight
			&& (Arc::ptr_eq(&self.message, &other.message) || self.message == other.message)
			&& same_rendered(&self.rendered, &other.rendered)
			&& (Arc::ptr_eq(&self.body, &other.body) || self.body == other.body)
			&& self.names == other.names
			&& self.reply == other.reply
			&& self.system == other.system
			&& self.look == other.look
			&& (Arc::ptr_eq(&self.history, &other.history) || self.history == other.history)
			&& self.interaction == other.interaction
			&& self.ephemeral == other.ephemeral
			&& self.reply_state == other.reply_state
			&& self.interaction_failed == other.interaction_failed
			&& self.waiting_component == other.waiting_component
			&& self.select_options == other.select_options
	}
}

fn same_rendered(left: &Option<Arc<RenderedMessage>>, right: &Option<Arc<RenderedMessage>>) -> bool {
	match (left, right) {
		(Some(left), Some(right)) => Arc::ptr_eq(left, right) || left == right,
		(None, None) => true,
		_ => false,
	}
}

impl MessageEntry {
	pub fn is_plain(&self) -> bool {
		is_plain(self.kind)
	}
}

fn is_plain(kind: MessageType) -> bool {
	matches!(
		kind,
		MessageType::Default
			| MessageType::Reply
			| MessageType::ChatInputCommand
			| MessageType::ContextMenuCommand
	)
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineRow {
	pub key: RowKey,
	pub row: Row,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delta {
	pub reset: bool,
	pub prepended: usize,
	pub appended: usize,
	pub inserted: Vec<RowKey>,
	pub updated: Vec<RowKey>,
	pub removed: Vec<RowKey>,
}

impl Delta {
	pub fn is_empty(&self) -> bool {
		!self.reset
			&& self.prepended == 0
			&& self.appended == 0
			&& self.inserted.is_empty()
			&& self.updated.is_empty()
			&& self.removed.is_empty()
	}

	pub fn merge(&mut self, later: Delta) {
		if later.reset {
			*self = later;
			return;
		}
		if self.reset {
			return;
		}
		self.prepended += later.prepended;
		self.appended += later.appended;
		for (mine, theirs) in [
			(&mut self.inserted, later.inserted),
			(&mut self.updated, later.updated),
			(&mut self.removed, later.removed),
		] {
			for key in theirs {
				if !mine.contains(&key) {
					mine.push(key);
				}
			}
		}
	}

	fn between(old: &[TimelineRow], new: &[TimelineRow]) -> Delta {
		let before: HashMap<&RowKey, &Row> = old.iter().map(|row| (&row.key, &row.row)).collect();
		let after: HashSet<&RowKey> = new.iter().map(|row| &row.key).collect();
		let common_message = |row: &&TimelineRow| {
			matches!(row.key, RowKey::Message(_) | RowKey::Pending(_)) && before.contains_key(&row.key)
		};
		let first_common = new.iter().position(|row| common_message(&row));
		let last_common = new.iter().rposition(|row| common_message(&row));
		let (Some(first_common), Some(last_common)) = (first_common, last_common) else {
			return Delta {
				reset: true,
				..Delta::default()
			};
		};
		let mut delta = Delta::default();
		for (index, row) in new.iter().enumerate() {
			match before.get(&row.key) {
				None if index < first_common => delta.prepended += 1,
				None if index > last_common => delta.appended += 1,
				None => delta.inserted.push(row.key.clone()),
				Some(previous) if **previous != row.row => delta.updated.push(row.key.clone()),
				Some(_) => {}
			}
		}
		delta.removed = old
			.iter()
			.filter(|row| !after.contains(&row.key))
			.map(|row| row.key.clone())
			.collect();
		delta
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenChannel {
	pub guild_id: Option<Snowflake>,
	pub channel_id: Snowflake,
}

type Parsed = (Arc<Message>, Option<Arc<RenderedMessage>>, Arc<Body>);
type History = (Arc<[MessageVersion]>, Arc<[Document]>);

#[derive(Debug, Default)]
pub struct Timeline {
	open: Option<OpenChannel>,
	rows: Vec<TimelineRow>,
	detached: bool,
	pending: Delta,
	bodies: HashMap<Snowflake, Parsed>,
	histories: HashMap<Snowflake, History>,
	display: Display,
}

impl Timeline {
	pub fn open(&self) -> Option<OpenChannel> {
		self.open
	}

	pub fn rows(&self) -> &[TimelineRow] {
		&self.rows
	}

	pub fn messages(&self) -> impl DoubleEndedIterator<Item = &MessageEntry> {
		self.rows.iter().filter_map(|row| match &row.row {
			Row::Message(entry) => Some(entry.as_ref()),
			_ => None,
		})
	}

	pub fn is_detached(&self) -> bool {
		self.detached
	}

	pub fn has_older(&self) -> bool {
		matches!(self.rows.first().map(|row| &row.row), Some(Row::Loading))
	}

	pub fn set_display(&mut self, display: Display) {
		self.display = display;
	}

	pub fn redraw(&mut self, state: &State, formatting: Formatting) -> bool {
		let Some(open) = self.open else {
			return false;
		};
		if state.channel(open.channel_id).is_none() {
			return false;
		}
		let fresh = self.project(state, open);
		let mut delta = if fresh == self.rows {
			Delta::default()
		} else {
			Delta::between(&self.rows, &fresh)
		};
		if formatting == Formatting::RelativeTimes && !delta.reset {
			let timed = fresh.iter().filter(|row| match &row.row {
				Row::Message(entry) => entry
					.body
					.documents()
					.any(|document| document.has_relative_timestamp()),
				_ => false,
			});
			for row in timed {
				if !delta.updated.contains(&row.key) && !delta.inserted.contains(&row.key) {
					delta.updated.push(row.key.clone());
				}
			}
		}
		self.rows = fresh;
		if delta.is_empty() {
			return false;
		}
		self.pending.merge(delta);
		true
	}

	pub fn take_delta(&mut self) -> Option<Delta> {
		let delta = std::mem::take(&mut self.pending);
		(!delta.is_empty()).then_some(delta)
	}

	pub fn select(
		&mut self,
		state: &State,
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	) -> Option<Command> {
		let channel = OpenChannel { guild_id, channel_id };
		if self.open == Some(channel) || state.channel(channel_id).is_none() {
			return None;
		}
		self.open = Some(channel);
		self.bodies.clear();
		self.histories.clear();
		self.rows = self.project(state, channel);
		self.detached = detached(state, channel_id);
		self.record_reset();
		Some(Command::SelectChannel { guild_id, channel_id })
	}

	pub fn close(&mut self) -> bool {
		let was_open = self.open.take().is_some();
		let had_rows = !std::mem::take(&mut self.rows).is_empty();
		self.bodies.clear();
		self.histories.clear();
		self.detached = false;
		if was_open || had_rows {
			self.record_reset();
		}
		had_rows
	}

	pub fn apply(&mut self, state: &State, changes: &Changes) -> bool {
		let Some(open) = self.open else {
			return false;
		};
		if state.channel(open.channel_id).is_none() {
			return self.close();
		}
		let guild = open.guild_id.unwrap_or_default();
		if !changes.messages.contains(open.channel_id)
			&& !changes.members.contains(guild)
			&& !changes.channel_lists.contains(guild)
			&& !changes.current_user
			&& !self.decorated_presence(changes)
		{
			return false;
		}
		self.detached = detached(state, open.channel_id);
		let fresh = self.project(state, open);
		if fresh == self.rows {
			return false;
		}
		let delta = Delta::between(&self.rows, &fresh);
		self.rows = fresh;
		self.pending.merge(delta);
		true
	}

	fn decorated_presence(&self, changes: &Changes) -> bool {
		!changes.presence.is_empty()
			&& self.display.formats(Formatting::Decorations)
			&& self
				.messages()
				.any(|entry| changes.presence.contains(entry.message.author.id))
	}

	fn record_reset(&mut self) {
		self.pending = Delta {
			reset: true,
			..Delta::default()
		};
	}
}

fn detached(state: &State, channel_id: Snowflake) -> bool {
	state
		.messages(channel_id)
		.is_some_and(|cache| cache.is_detached())
}

impl Timeline {
	fn project(&mut self, state: &State, open: OpenChannel) -> Vec<TimelineRow> {
		let mut bodies = HashMap::with_capacity(self.bodies.len());
		let mut histories = HashMap::new();
		let mut rows = Vec::new();
		let mut ungroup = false;
		for row in state.channel_rows(open.channel_id) {
			let MessageRow::Message(view) = row else {
				ungroup = false;
				rows.extend(marker_row(row));
				continue;
			};
			let look = self.display.look(state, open.guild_id, &view);
			if view.deleted.is_some()
				&& look
					.as_ref()
					.is_some_and(|look| look.deleted == Some(DeletedLook::Hidden))
			{
				ungroup |= !view.grouped_with_previous;
				continue;
			}
			let body = self.body_of(&view);
			bodies.insert(
				view.message.id,
				(view.message.clone(), view.rendered.clone(), body.clone()),
			);
			let history = match look.as_ref().and_then(|look| look.history) {
				Some(_) if !view.versions.is_empty() => {
					let history = self.history_of(&view);
					histories.insert(view.message.id, (view.versions.clone(), history.clone()));
					history
				}
				_ => Arc::from([]),
			};
			let mut entry = entry(state, open.guild_id, *view, body, &self.display);
			if !history.is_empty() {
				let documents = entry.body.documents().chain(history.iter());
				entry.names = Arc::new(Names::of(state, open.guild_id, &entry.message, documents));
			}
			entry.look = look;
			entry.history = history;
			entry.grouped &= !ungroup;
			ungroup = false;
			rows.push(TimelineRow {
				key: message_key(&entry),
				row: Row::Message(Box::new(entry)),
			});
		}
		self.bodies = bodies;
		self.histories = histories;
		rows
	}

	fn history_of(&self, view: &MessageView) -> Arc<[Document]> {
		match self.histories.get(&view.message.id) {
			Some((versions, history)) if Arc::ptr_eq(versions, &view.versions) => history.clone(),
			_ => view
				.versions
				.iter()
				.map(|version| markdown::parse(&version.content))
				.collect(),
		}
	}

	fn body_of(&self, view: &MessageView) -> Arc<Body> {
		match self.bodies.get(&view.message.id) {
			Some((message, rendered, body))
				if Arc::ptr_eq(message, &view.message) && same_rendered(rendered, &view.rendered) =>
			{
				body.clone()
			}
			_ => Arc::new(Body::parse(&view.message, view.rendered.as_deref())),
		}
	}
}

fn marker_row(row: MessageRow) -> Option<TimelineRow> {
	Some(match row {
		MessageRow::Loading => TimelineRow {
			key: RowKey::Loading,
			row: Row::Loading,
		},
		MessageRow::HistoryStart => TimelineRow {
			key: RowKey::Start,
			row: Row::Start,
		},
		MessageRow::DaySeparator(day) => TimelineRow {
			key: RowKey::Day(day),
			row: Row::Day(day),
		},
		MessageRow::UnreadMarker => TimelineRow {
			key: RowKey::Unread,
			row: Row::Unread,
		},
		MessageRow::Message(_) => return None,
	})
}

fn message_key(entry: &MessageEntry) -> RowKey {
	match (&entry.message.nonce, entry.pending) {
		(Some(nonce), true) => RowKey::Pending(nonce.0.clone()),
		_ => RowKey::Message(entry.id),
	}
}

fn author_name(
	state: &State,
	guild_id: Option<Snowflake>,
	message: &Message,
	display: &Display,
) -> Option<ShownName> {
	if !is_plain(message.kind) || !display.formats(Formatting::Names) {
		return None;
	}
	let mut author = person(state, guild_id, &message.author);
	if author.nick.is_none() {
		author.nick = message.member.as_ref().and_then(|member| member.nick.clone());
	}
	display.name(&author)
}

fn author_decorations(
	state: &State,
	guild_id: Option<Snowflake>,
	message: &Message,
	display: &Display,
) -> Vec<Badge> {
	if !is_plain(message.kind) || !display.formats(Formatting::Decorations) {
		return Vec::new();
	}
	display
		.decorations(state, guild_id, &message.author)
		.map(|decorations| decorations.messages)
		.unwrap_or_default()
}

fn entry(
	state: &State,
	guild_id: Option<Snowflake>,
	view: MessageView,
	body: Arc<Body>,
	display: &Display,
) -> MessageEntry {
	let reply_state = reply_state(&view, Utc::now());
	let message = view.message;
	crate::components::note_unknown(&message.components);
	let shown = author_name(state, guild_id, &message, display);
	let decorations = author_decorations(state, guild_id, &message, display);
	let names = Names::of(state, guild_id, &message, body.documents());
	let reply = ReplyPreview::of(state, guild_id, &message);
	let interaction = interaction_line(state, guild_id, &message);
	let system = (!is_plain(message.kind)).then(|| {
		let target = message
			.mentions
			.first()
			.map(|user| person_name(state, guild_id, user))
			.unwrap_or_default();
		system_line(message.kind, &view.author_display, &target, &message.content)
	});
	MessageEntry {
		body,
		names: Arc::new(names),
		reply,
		system,
		look: None,
		history: Arc::from([]),
		id: message.id,
		author_extra: shown.as_ref().and_then(|shown| shown.extra.clone()),
		author: shown.map_or(view.author_display, |shown| shown.name),
		author_color: view.author_color,
		decorations,
		content: message.content.clone(),
		sent_at: message.timestamp.unwrap_or_else(|| message.id.timestamp()),
		kind: message.kind,
		grouped: view.grouped_with_previous,
		edited: message.edited_timestamp.is_some(),
		pending: view.pending,
		failed: view.failure.is_some(),
		upload: view.upload,
		deleted: view.deleted.is_some(),
		bot: message.author.bot == Some(true),
		mine: state.current_user_id() == Some(message.author.id),
		highlight: highlight(state, guild_id, &message),
		ephemeral: message.is_ephemeral() || matches!(reply_state, Some(ReplyState::Failed(_))),
		interaction,
		reply_state,
		interaction_failed: view.interaction_failed,
		waiting_component: view.waiting_component,
		select_options: Arc::new(crate::components::select_options(
			state,
			guild_id,
			&message.components,
		)),
		rendered: view.rendered,
		message,
	}
}

pub fn detached_entry(state: &State, guild_id: Option<Snowflake>, message: &Arc<Message>) -> MessageEntry {
	let guild_id = message
		.guild_id
		.or(guild_id)
		.or_else(|| state.channel_guild(message.channel_id));
	let view = state.detached_view(guild_id, message);
	let body = Arc::new(Body::parse(&view.message, view.rendered.as_deref()));
	entry(state, guild_id, view, body, &Display::default())
}

pub fn busiest_channel(state: &State) -> Option<OpenChannel> {
	state
		.guild_summaries()
		.into_iter()
		.flat_map(|guild| {
			openable_channels(state, guild.id)
				.filter_map(|channel_id| {
					let last = state.channel(channel_id)?.last_message_id?;
					Some((last, guild.id, channel_id))
				})
				.collect::<Vec<_>>()
		})
		.max_by_key(|(last, _, _)| *last)
		.map(|(_, guild_id, channel_id)| OpenChannel {
			guild_id: Some(guild_id),
			channel_id,
		})
}

pub fn first_channel(state: &State) -> Option<OpenChannel> {
	let guild_id = state.guild_summaries().first()?.id;
	let channel_id = openable_channels(state, guild_id).next()?;
	Some(OpenChannel {
		guild_id: Some(guild_id),
		channel_id,
	})
}

pub fn replacement_channel(state: &State, open: OpenChannel) -> Option<OpenChannel> {
	let guild_id = open.guild_id?;
	if state.channel(open.channel_id).is_some() || state.guild(guild_id).is_none() {
		return None;
	}
	let channel_id = openable_channels(state, guild_id).next()?;
	Some(OpenChannel {
		guild_id: Some(guild_id),
		channel_id,
	})
}

fn openable_channels(state: &State, guild_id: Snowflake) -> impl Iterator<Item = Snowflake> {
	state
		.guild_channel_tree(guild_id)
		.into_iter()
		.flat_map(|node| node.children)
		.filter(|row| !row.locked)
		.filter(|row| {
			matches!(
				ChannelKind::of(row.kind),
				ChannelKind::Text | ChannelKind::Announcement
			)
		})
		.map(|row| row.id)
}
