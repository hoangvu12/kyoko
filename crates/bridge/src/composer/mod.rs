mod marks;
mod slash;
mod suggest;

use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use protocol::{Gif, Nonce, Snowflake};
use rest::UploadFile;
use session::Command;
use state::{MAX_ATTACHMENTS, RiskyLock, SendFailure, State};

use crate::emoji_picker::EmojiCell;
use crate::sticker_picker::StickerCell;

pub use marks::{LineState, Mark, Marked, line_marks, text_marks};
pub use slash::{Slash, parse_slash};
pub use suggest::{
	Query, Suggestion, Trigger, Visual, apply_suggestion, emoji_candidates, placeholder, query_at,
	suggestions, typing_line,
};

const MIB: f64 = 1024.0 * 1024.0;
const MEMBER_QUERY: usize = 2;
const REJECTION_WINDOW: TimeDelta = TimeDelta::seconds(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
	pub message_id: Snowflake,
	pub name: String,
	pub ping: bool,
	pub own: bool,
}

#[derive(Debug, Clone)]
pub struct Attachment {
	pub id: u64,
	pub file: UploadFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
	Lock { text: String, confirmable: bool },
	Failed { nonce: Nonce, text: String },
	Refused(String),
}

#[derive(Debug, Default)]
pub struct Outcome {
	pub commands: Vec<Command>,
	pub replace: Option<(String, usize)>,
}

impl Outcome {
	fn command(command: Command) -> Outcome {
		Outcome {
			commands: vec![command],
			replace: None,
		}
	}

	fn cleared(commands: Vec<Command>) -> Outcome {
		Outcome {
			commands,
			replace: Some((String::new(), 0)),
		}
	}
}

#[derive(Default)]
pub struct Composer {
	guild_id: Option<Snowflake>,
	channel_id: Option<Snowflake>,
	reply: Option<Reply>,
	editing: Option<Snowflake>,
	files: Vec<Attachment>,
	next_file: u64,
	query: Option<Query>,
	suggestions: Vec<Suggestion>,
	selected: usize,
	held: Option<Command>,
	refusal: Option<String>,
	seen_rejection: u64,
	acted_at: Option<DateTime<Utc>>,
	member_query: Option<String>,
	ranked_for: Option<(String, u64)>,
	dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileEdit {
	pub name: String,
	pub description: String,
	pub spoiler: bool,
}

pub fn too_big(name: &str, size: u64, limit: u64) -> Option<String> {
	(size > limit).then(|| {
		format!(
			"{name} is too big ({}). The limit here is {} MB.",
			crate::file_size(size).replace(".0 ", " "),
			(limit as f64 / MIB).round()
		)
	})
}

fn leaves_draft(command: &Command) -> bool {
	match command {
		Command::SendGif { .. } | Command::SendSticker { .. } => true,
		Command::SendMessage { content, .. } => content.is_empty(),
		_ => false,
	}
}

pub fn failure_text(failure: &SendFailure) -> String {
	match failure {
		SendFailure::RateLimited { retry_after } => format!(
			"You are sending messages too quickly. Wait {} seconds before retrying.",
			retry_after.as_secs().max(1)
		),
		SendFailure::Forbidden => "Discord refused this message.".to_owned(),
		SendFailure::MissingPermissions => "You do not have permission to send this here.".to_owned(),
		SendFailure::Captcha { .. } => {
			"Discord asked for a captcha. Complete this in the official client.".to_owned()
		}
		SendFailure::Unauthorized => "You are signed out.".to_owned(),
		SendFailure::Network => "Could not reach Discord.".to_owned(),
		SendFailure::Other(Some(status)) => format!("The message could not be sent (error {status})."),
		SendFailure::Other(None) => "The message could not be sent.".to_owned(),
	}
}

impl Composer {
	pub fn channel(&self) -> Option<Snowflake> {
		self.channel_id
	}

	pub fn guild(&self) -> Option<Snowflake> {
		self.guild_id
	}

	pub fn reply(&self) -> Option<&Reply> {
		self.reply.as_ref()
	}

	pub fn editing(&self) -> Option<Snowflake> {
		self.editing
	}

	pub fn files(&self) -> &[Attachment] {
		&self.files
	}

	pub fn suggestions(&self) -> &[Suggestion] {
		&self.suggestions
	}

	pub fn selected(&self) -> usize {
		self.selected
	}

	pub fn is_dirty(&self) -> bool {
		self.dirty
	}

	pub fn open(
		&mut self,
		state: &State,
		guild_id: Option<Snowflake>,
		channel_id: Option<Snowflake>,
	) -> Outcome {
		if self.channel_id == channel_id {
			return Outcome::default();
		}
		*self = Composer {
			next_file: self.next_file,
			seen_rejection: state.last_rejection().map_or(0, |rejection| rejection.id),
			guild_id,
			channel_id,
			..Composer::default()
		};
		let draft = channel_id
			.and_then(|channel_id| state.draft(channel_id))
			.unwrap_or_default()
			.to_owned();
		let caret = draft.len();
		Outcome {
			commands: Vec::new(),
			replace: Some((draft, caret)),
		}
	}

	pub fn draft_arrived(&mut self, state: &State) -> Option<(String, usize)> {
		if self.dirty || self.editing.is_some() {
			return None;
		}
		let draft = state.draft(self.channel_id?)?.to_owned();
		let caret = draft.len();
		Some((draft, caret))
	}

	pub fn restored(&mut self, state: &State, text: &str, caret: usize) {
		self.update_query(state, text, caret);
	}

	fn update_query(&mut self, state: &State, text: &str, caret: usize) {
		self.query = query_at(text, caret);
		self.selected = 0;
		self.suggestions = match (&self.query, self.channel_id) {
			(Some(query), Some(channel_id)) => suggestions(state, self.guild_id, channel_id, query),
			_ => Vec::new(),
		};
	}

	pub fn changed(&mut self, state: &State, text: &str, caret: usize) -> Vec<Command> {
		self.dirty = true;
		let Some(channel_id) = self.channel_id else {
			return Vec::new();
		};
		let mut commands = Vec::new();
		if !text.is_empty() && self.editing.is_none() {
			commands.push(Command::Typing { channel_id });
		}
		self.update_query(state, text, caret);
		match &self.query {
			Some(query) if query.trigger == Trigger::Command && state.command_index(channel_id).is_none() => {
				commands.push(Command::LoadApplicationCommands { channel_id });
			}
			Some(query)
				if query.trigger == Trigger::Mention
					&& query.text.chars().count() >= MEMBER_QUERY
					&& self.member_query.as_deref() != Some(query.text.as_str()) =>
			{
				if let Some(guild_id) = self.guild_id {
					self.member_query = Some(query.text.clone());
					commands.push(Command::SearchMembers {
						guild_id,
						query: query.text.clone(),
					});
				}
			}
			_ => {}
		}
		commands.extend(self.rank_request(state));
		commands
	}

	pub fn rank_request(&mut self, state: &State) -> Option<Command> {
		let query = self
			.query
			.as_ref()
			.filter(|query| query.trigger == Trigger::Emoji)?;
		let asked = (query.text.clone(), state.favorites_generation());
		if self.ranked_for.as_ref() == Some(&asked) {
			return None;
		}
		self.ranked_for = Some(asked);
		Some(Command::RankEmoji {
			guild_id: self.guild_id,
			query: query.text.clone(),
			candidates: emoji_candidates(state, self.guild_id, query),
		})
	}

	pub fn refresh_suggestions(&mut self, state: &State) -> bool {
		let (Some(channel_id), Some(query)) = (self.channel_id, &self.query) else {
			return false;
		};
		let fresh = suggestions(state, self.guild_id, channel_id, query);
		if fresh == self.suggestions {
			return false;
		}
		self.suggestions = fresh;
		self.selected = self.selected.min(self.suggestions.len().saturating_sub(1));
		true
	}

	pub fn move_selection(&mut self, delta: i32) {
		let count = self.suggestions.len();
		if count == 0 {
			return;
		}
		let step = delta.unsigned_abs() as usize % count;
		self.selected = if delta < 0 {
			(self.selected + count - step) % count
		} else {
			(self.selected + step) % count
		};
	}

	pub fn choose(&mut self, text: &str, index: Option<usize>) -> Option<(String, usize)> {
		let query = self.query.take()?;
		let suggestion = self.suggestions.get(index.unwrap_or(self.selected))?.clone();
		self.suggestions.clear();
		self.selected = 0;
		Some(apply_suggestion(text, &query, &suggestion))
	}

	pub fn dismiss(&mut self) -> bool {
		if !self.suggestions.is_empty() {
			self.suggestions.clear();
			self.query = None;
			return true;
		}
		if self.refusal.take().is_some() {
			return true;
		}
		if self.reply.take().is_some() {
			return true;
		}
		false
	}

	pub fn start_reply(&mut self, state: &State, message_id: Snowflake) -> bool {
		let Some(channel_id) = self.channel_id else {
			return false;
		};
		let Some(message) = state
			.messages(channel_id)
			.and_then(|cache| cache.by_id.get(&message_id))
		else {
			return false;
		};
		let own = state.current_user_id() == Some(message.author.id);
		self.editing = None;
		self.reply = Some(Reply {
			message_id,
			name: state.display_name(self.guild_id, message.author.id),
			ping: !own,
			own,
		});
		true
	}

	pub fn cancel_reply(&mut self) {
		self.reply = None;
	}

	pub fn toggle_ping(&mut self) {
		if let Some(reply) = &mut self.reply
			&& !reply.own
		{
			reply.ping = !reply.ping;
		}
	}

	pub fn start_edit(&mut self, state: &State, message_id: Snowflake) -> Option<(String, usize)> {
		let channel_id = self.channel_id?;
		let message = state.messages(channel_id)?.by_id.get(&message_id)?;
		if state.current_user_id() != Some(message.author.id) {
			return None;
		}
		self.editing = Some(message_id);
		self.reply = None;
		self.suggestions.clear();
		self.query = None;
		let content = message.content.clone();
		let caret = content.len();
		Some((content, caret))
	}

	pub fn edit_last(&mut self, state: &State) -> Option<(String, usize)> {
		let channel_id = self.channel_id?;
		let me = state.current_user_id()?;
		let message_id = state
			.messages(channel_id)?
			.ordered
			.iter()
			.rev()
			.find(|message| message.author.id == me)
			.map(|message| message.id)?;
		self.start_edit(state, message_id)
	}

	pub fn cancel_edit(&mut self, state: &State) -> Option<(String, usize)> {
		self.editing.take()?;
		let draft = self
			.channel_id
			.and_then(|channel_id| state.draft(channel_id))
			.unwrap_or_default()
			.to_owned();
		let caret = draft.len();
		Some((draft, caret))
	}

	pub fn attach(&mut self, state: &State, file: UploadFile) -> Result<u64, String> {
		let channel_id = self.channel_id.ok_or_else(|| "Open a channel first".to_owned())?;
		if self.files.len() >= MAX_ATTACHMENTS {
			return Err(format!("You can attach up to {MAX_ATTACHMENTS} files"));
		}
		if let Some(refusal) = too_big(&file.name, file.size(), state.upload_limit(channel_id)) {
			return Err(refusal);
		}
		self.next_file += 1;
		let id = self.next_file;
		self.files.push(Attachment { id, file });
		self.refusal = None;
		Ok(id)
	}

	pub fn toggle_spoiler(&mut self, id: u64) -> bool {
		let Some(attachment) = self.files.iter_mut().find(|attachment| attachment.id == id) else {
			return false;
		};
		attachment.file.spoiler = !attachment.file.spoiler;
		true
	}

	pub fn file(&self, id: u64) -> Option<&Attachment> {
		self.files.iter().find(|attachment| attachment.id == id)
	}

	pub fn edit_file(&mut self, id: u64, edit: FileEdit) -> bool {
		let Some(attachment) = self.files.iter_mut().find(|attachment| attachment.id == id) else {
			return false;
		};
		let name = edit.name.trim();
		if !name.is_empty() {
			attachment.file.name = name.to_owned();
		}
		let description = edit.description.trim();
		attachment.file.description = (!description.is_empty()).then(|| description.to_owned());
		attachment.file.spoiler = edit.spoiler;
		true
	}

	pub fn upload_now(&mut self, state: &State, files: Vec<UploadFile>, now: DateTime<Utc>) -> Outcome {
		let Some(channel_id) = self.channel_id else {
			return Outcome::default();
		};
		if files.is_empty() {
			return Outcome::default();
		}
		if files.len() > MAX_ATTACHMENTS {
			self.refusal = Some(format!("You can attach up to {MAX_ATTACHMENTS} files"));
			return Outcome::default();
		}
		let limit = state.upload_limit(channel_id);
		if let Some(refusal) = files
			.iter()
			.find_map(|file| too_big(&file.name, file.size(), limit))
		{
			self.refusal = Some(refusal);
			return Outcome::default();
		}
		self.expression(
			state,
			Command::SendMessage {
				channel_id,
				content: String::new(),
				reply_to: None,
				ping_reply: true,
				files,
			},
			now,
		)
	}

	pub fn refuse(&mut self, reason: String) {
		self.refusal = Some(reason);
	}

	pub fn remove_file(&mut self, id: u64) -> bool {
		let before = self.files.len();
		self.files.retain(|attachment| attachment.id != id);
		self.files.len() != before
	}

	pub fn lock(&self, state: &State, now: DateTime<Utc>) -> Option<RiskyLock> {
		state.channel_lock(self.channel_id?, now)
	}

	pub fn acted_recently(&self, now: DateTime<Utc>) -> bool {
		self.acted_at
			.is_some_and(|at| now.signed_duration_since(at) < REJECTION_WINDOW)
	}

	pub fn notice(&mut self, state: &State, now: DateTime<Utc>) -> Option<Notice> {
		if let Some(rejection) = state.last_rejection()
			&& rejection.id > self.seen_rejection
		{
			self.seen_rejection = rejection.id;
			if self.acted_recently(now) {
				self.refusal = Some(rejection.reason.clone());
			}
		}
		if let Some(lock) = self.lock(state, now) {
			return Some(Notice::Lock {
				text: lock.reason.explanation().to_owned(),
				confirmable: !lock.reason.is_gate(),
			});
		}
		if let Some(refusal) = &self.refusal {
			return Some(Notice::Refused(refusal.clone()));
		}
		let channel_id = self.channel_id?;
		state
			.messages(channel_id)?
			.pending
			.iter()
			.rev()
			.filter(|pending| pending.upload.is_none())
			.find_map(|pending| {
				let failure = pending.failure.as_ref()?;
				Some(Notice::Failed {
					nonce: pending.message.nonce.clone()?,
					text: failure_text(failure),
				})
			})
	}

	pub fn confirm_lock(&mut self, state: &State, now: DateTime<Utc>) -> Outcome {
		let Some(lock) = self.lock(state, now) else {
			return Outcome::default();
		};
		self.acted_at = Some(now);
		let mut commands = vec![Command::ConfirmRisky {
			target: lock.target,
			reason: lock.reason,
		}];
		match self.held.take() {
			Some(held) if leaves_draft(&held) => {
				commands.push(held);
				Outcome {
					commands,
					replace: None,
				}
			}
			Some(held) => {
				commands.push(held);
				self.reply = None;
				self.files.clear();
				Outcome::cleared(commands)
			}
			None => Outcome {
				commands,
				replace: None,
			},
		}
	}

	pub fn retry(&self, nonce: Nonce) -> Option<Command> {
		Some(Command::RetryMessage {
			channel_id: self.channel_id?,
			nonce,
		})
	}

	pub fn discard(&self, nonce: Nonce) -> Option<Command> {
		Some(Command::DiscardMessage {
			channel_id: self.channel_id?,
			nonce,
		})
	}

	pub fn submit(&mut self, state: &State, text: &str, now: DateTime<Utc>) -> Outcome {
		let Some(channel_id) = self.channel_id else {
			return Outcome::default();
		};
		self.refusal = None;
		self.acted_at = Some(now);
		let content = text.trim();
		if let Some(message_id) = self.editing {
			if content.is_empty() {
				return Outcome::default();
			}
			self.editing = None;
			self.dirty = false;
			let mut outcome = Outcome::command(Command::EditMessage {
				channel_id,
				message_id,
				content: content.to_owned(),
			});
			let draft = state.draft(channel_id).unwrap_or_default().to_owned();
			let caret = draft.len();
			outcome.replace = Some((draft, caret));
			return outcome;
		}
		if content.is_empty() && self.files.is_empty() {
			return Outcome::default();
		}
		let limit = state.message_limit();
		let length = content.chars().count();
		if length > limit {
			self.refusal = Some(format!(
				"Your message is too long ({length} of {limit} characters)."
			));
			return Outcome::default();
		}
		let command = match self.files.is_empty() {
			true => match parse_slash(state.command_index(channel_id).map(AsRef::as_ref), content) {
				Slash::Run { command_id, options } => Command::RunApplicationCommand {
					channel_id,
					command_id,
					options,
				},
				Slash::Invalid(reason) => {
					self.refusal = Some(reason);
					return Outcome::default();
				}
				Slash::Text => self.message(channel_id, content),
			},
			false => self.message(channel_id, content),
		};
		if self.lock(state, now).is_some() {
			self.held = Some(command);
			return Outcome::default();
		}
		self.reply = None;
		self.files.clear();
		self.suggestions.clear();
		self.query = None;
		self.dirty = false;
		Outcome::cleared(vec![
			command,
			Command::SetDraft {
				channel_id,
				content: String::new(),
			},
		])
	}

	pub fn pick_emoji(&mut self, text: &str, caret: usize, cell: &EmojiCell) -> Option<(String, usize)> {
		if cell.locked || self.channel_id.is_none() {
			return None;
		}
		let mut at = caret.min(text.len());
		while !text.is_char_boundary(at) {
			at -= 1;
		}
		let mut inserted = String::with_capacity(text.len() + cell.markup.len());
		inserted.push_str(&text[..at]);
		inserted.push_str(&cell.markup);
		inserted.push_str(&text[at..]);
		self.dirty = true;
		Some((inserted, at + cell.markup.len()))
	}

	pub fn pick_gif(&mut self, state: &State, gif: Gif, usage: Option<&str>, now: DateTime<Utc>) -> Outcome {
		let Some(channel_id) = self.channel_id else {
			return Outcome::default();
		};
		let mut outcome = self.expression(
			state,
			Command::SendGif {
				channel_id,
				gif: gif.clone(),
			},
			now,
		);
		if let Some(query) = usage
			&& !outcome.commands.is_empty()
		{
			outcome.commands.insert(
				0,
				Command::SelectGif {
					id: gif.id,
					query: query.to_owned(),
				},
			);
		}
		outcome
	}

	pub fn pick_sticker(&mut self, state: &State, cell: &StickerCell, now: DateTime<Utc>) -> Outcome {
		let Some(channel_id) = self.channel_id.filter(|_| !cell.locked) else {
			return Outcome::default();
		};
		self.expression(
			state,
			Command::SendSticker {
				channel_id,
				sticker_id: cell.sticker.id,
			},
			now,
		)
	}

	fn expression(&mut self, state: &State, command: Command, now: DateTime<Utc>) -> Outcome {
		self.refusal = None;
		self.acted_at = Some(now);
		if self.lock(state, now).is_some() {
			self.held = Some(command);
			return Outcome::default();
		}
		Outcome::command(command)
	}

	fn message(&self, channel_id: Snowflake, content: &str) -> Command {
		Command::SendMessage {
			channel_id,
			content: content.to_owned(),
			reply_to: self.reply.as_ref().map(|reply| reply.message_id),
			ping_reply: self.reply.as_ref().is_none_or(|reply| reply.ping),
			files: self
				.files
				.iter()
				.map(|attachment| attachment.file.clone())
				.collect(),
		}
	}

	pub fn typing(&self, state: &State, now: Instant) -> String {
		self.channel_id
			.map(|channel_id| typing_line(state, self.guild_id, channel_id, now))
			.unwrap_or_default()
	}
}
