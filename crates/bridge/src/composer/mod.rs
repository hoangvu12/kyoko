mod apps;
mod command;
mod marks;
mod suggest;

use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use protocol::{Gif, Nonce, Snowflake};
use rest::UploadFile;
use session::Command;
use state::{MAX_ATTACHMENTS, RiskyLock, SendFailure, State};

use crate::emoji_picker::EmojiCell;
use crate::sticker_picker::StickerCell;

pub use apps::{AppSearch, AppsLauncher, LauncherApp, RECENT_TILES, apps_launcher, search_apps};
pub use command::{
	ActiveCommand, BuiltIn, ChoiceList, CommandApp, CommandEntry, CommandHint, CommandPicker, CommandSection,
	CommandSource, OptionChoices, OptionField, OptionFile, Prepared, command_catalog, command_picker,
	exact_command, interaction_options, option_value,
};
pub use marks::{LineState, Mark, Marked, line_marks, text_marks};
pub use suggest::{
	Query, Suggestion, Trigger, Visual, apply_suggestion, emoji_candidates, placeholder, query_at,
	suggestion_heading, suggestions, typing_line,
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
	picker: Option<CommandPicker>,
	command: Option<ActiveCommand>,
	app_filter: Option<Snowflake>,
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

fn now_ms() -> u64 {
	u64::try_from(Utc::now().timestamp_millis()).unwrap_or_default()
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

	pub fn heading(&self) -> Option<(&'static str, String)> {
		self.query
			.as_ref()
			.filter(|_| !self.suggestions.is_empty())
			.map(suggestion_heading)
	}

	pub fn is_dirty(&self) -> bool {
		self.dirty
	}

	pub fn picker(&self) -> Option<&CommandPicker> {
		self.picker.as_ref()
	}

	pub fn command(&self) -> Option<&ActiveCommand> {
		self.command.as_ref()
	}

	fn fresh_picker(&self, state: &State) -> Option<CommandPicker> {
		let channel_id = self.channel_id?;
		let query = self
			.query
			.as_ref()
			.filter(|query| query.trigger == Trigger::Command && self.command.is_none())?;
		let picker = command_picker(state, self.guild_id, channel_id, &query.text, now_ms());
		Some(match self.app_filter {
			Some(app_id) => picker.only_app(app_id),
			None => picker,
		})
	}

	pub fn app_filter(&self) -> Option<Snowflake> {
		self.app_filter
	}

	pub fn open_app(&mut self, app_id: Snowflake) -> Option<(String, usize)> {
		if self.editing.is_some() || self.channel_id.is_none() {
			return None;
		}
		self.command = None;
		self.app_filter = Some(app_id);
		self.dirty = true;
		Some(("/".to_owned(), 1))
	}

	pub fn pick_command(&mut self, entry: CommandEntry) -> bool {
		if self.editing.is_some() || self.channel_id.is_none() {
			return false;
		}
		self.activate(entry);
		true
	}

	fn activate(&mut self, entry: CommandEntry) {
		self.command = Some(ActiveCommand::new(entry));
		self.picker = None;
		self.app_filter = None;
		self.query = None;
		self.suggestions.clear();
		self.selected = 0;
		self.dirty = true;
	}

	pub fn exact_command(&mut self, state: &State, text: &str) -> bool {
		if self.command.is_some() || self.editing.is_some() {
			return false;
		}
		let Some(channel_id) = self.channel_id else {
			return false;
		};
		match exact_command(state, self.guild_id, channel_id, text) {
			Some(entry) => {
				self.activate(entry);
				true
			}
			None => false,
		}
	}

	pub fn choose_section(&mut self, section: usize) -> bool {
		self.picker.as_mut().is_some_and(|picker| picker.jump(section))
	}

	pub fn cancel_command(&mut self) -> bool {
		self.command.take().is_some()
	}

	pub fn leave_command(&mut self) -> Option<(String, usize)> {
		let command = self.command.take()?;
		let text = format!("/{}", command.entry.name());
		let caret = text.len();
		Some((text, caret))
	}

	pub fn edit_option(&mut self, index: usize, value: &str) -> bool {
		self.command
			.as_mut()
			.is_some_and(|command| command.edit(index, value))
	}

	pub fn focus_option(&mut self, index: usize) -> bool {
		self.command.as_mut().is_some_and(|command| {
			let moved = command.focus(index);
			if moved {
				command.optional_open = false;
			}
			moved
		})
	}

	pub fn step_option(&mut self, delta: i32) -> bool {
		self.command.as_mut().is_some_and(|command| command.step(delta))
	}

	pub fn toggle_optional(&mut self) -> bool {
		self.command.as_mut().is_some_and(ActiveCommand::toggle_optional)
	}

	pub fn add_optional(&mut self, index: Option<usize>) -> bool {
		self.command
			.as_mut()
			.is_some_and(|command| command.add_optional(index))
	}

	pub fn remove_option(&mut self, index: usize) -> bool {
		self.command
			.as_mut()
			.is_some_and(|command| command.remove_field(index))
	}

	pub fn option_choices(&self, state: &State) -> Option<OptionChoices> {
		self.command.as_ref()?.choices(state.autocomplete())
	}

	pub fn autocomplete_request(&mut self, retry: bool) -> Option<Command> {
		let channel_id = self.channel_id?;
		let (command_id, options, request) = self.command.as_mut()?.autocomplete(retry)?;
		Some(Command::AutocompleteCommand {
			channel_id,
			command_id,
			options,
			request,
		})
	}

	pub fn move_choice(&mut self, state: &State, delta: i32) -> bool {
		let Some(count) = self.option_choices(state).map(|choices| choices.count()) else {
			return false;
		};
		match self.command.as_mut() {
			Some(command) if count > 0 => {
				command.move_choice(count, delta);
				true
			}
			_ => false,
		}
	}

	pub fn pick_choice(&mut self, state: &State, index: Option<usize>) -> bool {
		let Some(choices) = self.option_choices(state) else {
			return false;
		};
		let choice = match (&choices.list, index) {
			(ChoiceList::Choices(list), Some(index)) => list.get(index).cloned(),
			_ => choices.selected_choice().cloned(),
		};
		match (choice, self.command.as_mut()) {
			(Some(choice), Some(command)) => {
				command.pick_choice(choice);
				true
			}
			_ => false,
		}
	}

	pub fn wants_file(&self) -> bool {
		self.command.as_ref().is_some_and(ActiveCommand::wants_file)
	}

	pub fn attach_option(&mut self, state: &State, file: UploadFile) -> Result<u64, String> {
		let channel_id = self.channel_id.ok_or_else(|| "Open a channel first".to_owned())?;
		if !self.wants_file() {
			return Err("Pick a file option first".to_owned());
		}
		if let Some(refusal) = too_big(&file.name, file.size(), state.upload_limit(channel_id)) {
			return Err(refusal);
		}
		self.next_file += 1;
		let id = self.next_file;
		let attached = self
			.command
			.as_mut()
			.is_some_and(|command| command.attach(OptionFile { id, file }));
		if attached {
			Ok(id)
		} else {
			Err("Pick a file option first".to_owned())
		}
	}

	pub fn clear_option_file(&mut self, index: usize) -> Option<u64> {
		self.command.as_mut()?.clear_file(index)
	}

	pub fn option_file_ids(&self) -> Vec<u64> {
		self.command
			.as_ref()
			.map(ActiveCommand::file_ids)
			.unwrap_or_default()
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
		if self
			.query
			.as_ref()
			.is_none_or(|query| query.trigger != Trigger::Command)
		{
			self.app_filter = None;
		}
		self.selected = 0;
		self.suggestions = match (&self.query, self.channel_id) {
			(Some(query), Some(channel_id)) => suggestions(state, self.guild_id, channel_id, query),
			_ => Vec::new(),
		};
		self.picker = self.fresh_picker(state);
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
		if let Some(mut fresh) = self.fresh_picker(state) {
			let same_rows = self.picker.as_ref().is_some_and(|picker| {
				picker.rows.len() == fresh.rows.len()
					&& picker
						.rows
						.iter()
						.zip(&fresh.rows)
						.all(|(old, new)| old.key == new.key)
			});
			if same_rows {
				fresh.selected = self.picker.as_ref().map_or(0, |picker| picker.selected);
			}
			if self.picker.as_ref() == Some(&fresh) {
				return false;
			}
			self.picker = Some(fresh);
			return true;
		}
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
		if let Some(picker) = &mut self.picker {
			picker.move_selection(delta);
			return;
		}
		if let Some(command) = self.command.as_mut().filter(|command| command.optional_open) {
			command.move_optional(delta);
			return;
		}
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
		if let Some(picker) = &self.picker {
			let entry = picker.rows.get(index.unwrap_or(picker.selected))?.clone();
			self.activate(entry);
			return Some((String::new(), 0));
		}
		let suggestion = self
			.suggestions
			.get(index.unwrap_or(self.selected))
			.filter(|suggestion| suggestion.lock.is_none())?
			.clone();
		let query = self.query.take()?;
		self.suggestions.clear();
		self.selected = 0;
		Some(apply_suggestion(text, &query, &suggestion))
	}

	pub fn dismiss(&mut self) -> bool {
		if self.picker.take().is_some() {
			self.query = None;
			self.app_filter = None;
			return true;
		}
		if let Some(command) = self.command.as_mut().filter(|command| command.optional_open) {
			command.optional_open = false;
			return true;
		}
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
		self.picker = None;
		self.command = None;
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
				if let Some(command) = self.command.take() {
					commands.push(Command::CountCommandUse {
						key: command.entry.key,
					});
				}
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
		if self.command.is_some() {
			return self.submit_command(state, channel_id, now);
		}
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
		let command = self.message(channel_id, content);
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

	fn submit_command(&mut self, state: &State, channel_id: Snowflake, now: DateTime<Utc>) -> Outcome {
		let Some(active) = self.command.as_mut() else {
			return Outcome::default();
		};
		let values = match active.prepare() {
			Prepared::Run(values) => values,
			Prepared::Invalid => return Outcome::default(),
		};
		let command = match &active.entry.source {
			CommandSource::Application { command_id, .. } => Command::RunApplicationCommand {
				channel_id,
				command_id: *command_id,
				options: interaction_options(&active.entry, values),
				files: active.files(),
			},
			CommandSource::BuiltIn(kind) => {
				let text = values
					.iter()
					.find(|(option, _)| option.name == "message")
					.and_then(|(_, value)| value.as_str())
					.unwrap_or_default();
				match kind.message(text) {
					Some(content) => Command::SendMessage {
						channel_id,
						content,
						reply_to: None,
						ping_reply: true,
						files: Vec::new(),
					},
					None => {
						active.refusal = Some(format!("kyoko can't run /{} yet", active.entry.name()));
						return Outcome::default();
					}
				}
			}
		};
		if self.lock(state, now).is_some() {
			self.held = Some(command);
			return Outcome::default();
		}
		let key = self
			.command
			.take()
			.map(|active| active.entry.key)
			.unwrap_or_default();
		self.dirty = false;
		Outcome::cleared(vec![
			command,
			Command::CountCommandUse { key },
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
