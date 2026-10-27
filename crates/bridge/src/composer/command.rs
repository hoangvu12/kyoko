use std::sync::atomic::{AtomicU64, Ordering};

use protocol::{
	ApplicationCommand, ApplicationCommandIndex, ApplicationCommandType, ChannelType, CommandChoice,
	CommandOption, CommandOptionType, InteractionOption, Permissions, Snowflake,
};
use rest::UploadFile;
use serde_json::{Number, Value};
use state::{Autocomplete, AutocompleteStatus, State};

use crate::guild_admin::GuildAccess;

const FREQUENT_ROWS: usize = 5;
const FREQUENTLY_USED: &str = "Frequently Used";
const BUILT_IN: &str = "Built-In";
const REQUIRED: &str = "This option is required";
const FILE_REQUIRED: &str = "Pick a file to upload";

static AUTOCOMPLETE_REQUESTS: AtomicU64 = AtomicU64::new(0);

fn next_autocomplete_request() -> u64 {
	AUTOCOMPLETE_REQUESTS.fetch_add(1, Ordering::Relaxed) + 1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltIn {
	Ban,
	Gif,
	Kick,
	Me,
	Msg,
	Nick,
	Shrug,
	Spoiler,
	Sticker,
	Tableflip,
	Thread,
	Timeout,
	Unflip,
}

#[derive(Clone, Copy)]
struct BuiltInOption {
	name: &'static str,
	description: &'static str,
	kind: CommandOptionType,
	required: bool,
	max_length: Option<i64>,
	choices: &'static [(&'static str, i64)],
}

const fn text_option(name: &'static str, description: &'static str, required: bool) -> BuiltInOption {
	BuiltInOption {
		name,
		description,
		kind: CommandOptionType::String,
		required,
		max_length: None,
		choices: &[],
	}
}

const fn limited(option: BuiltInOption, max_length: i64) -> BuiltInOption {
	BuiltInOption {
		max_length: Some(max_length),
		..option
	}
}

const fn user_option(description: &'static str) -> BuiltInOption {
	BuiltInOption {
		name: "user",
		description,
		kind: CommandOptionType::User,
		required: true,
		max_length: None,
		choices: &[],
	}
}

const fn choice_option(
	name: &'static str,
	description: &'static str,
	choices: &'static [(&'static str, i64)],
) -> BuiltInOption {
	BuiltInOption {
		name,
		description,
		kind: CommandOptionType::Integer,
		required: true,
		max_length: None,
		choices,
	}
}

struct BuiltInSpec {
	kind: BuiltIn,
	key: &'static str,
	name: &'static str,
	description: &'static str,
	options: &'static [BuiltInOption],
}

const NICK_LIMIT: i64 = 32;
const THREAD_NAME_LIMIT: i64 = 100;

pub const DELETE_MESSAGES: [(&str, i64); 7] = [
	("Don't Delete Any", 0),
	("Previous Hour", 3600),
	("Previous 6 Hours", 21_600),
	("Previous 12 Hours", 43_200),
	("Previous 24 Hours", 86_400),
	("Previous 3 Days", 259_200),
	("Previous 7 Days", 604_800),
];

pub const TIMEOUT_DURATIONS: [(&str, i64); 6] = [
	("60 secs", 60),
	("5 mins", 300),
	("10 mins", 600),
	("1 hour", 3600),
	("1 day", 86_400),
	("1 week", 604_800),
];

const BUILT_INS: [BuiltInSpec; 13] = [
	BuiltInSpec {
		kind: BuiltIn::Ban,
		key: "-12",
		name: "ban",
		description: "Ban user",
		options: &[
			user_option("The user to ban"),
			choice_option(
				"delete_messages",
				"How much of their recent message history to delete",
				&DELETE_MESSAGES,
			),
			text_option("reason", "The reason for banning, if any", false),
		],
	},
	BuiltInSpec {
		kind: BuiltIn::Gif,
		key: "-16",
		name: "gif",
		description: "Search Animated GIFs on the Web",
		options: &[text_option("query", "Search for a GIF", true)],
	},
	BuiltInSpec {
		kind: BuiltIn::Kick,
		key: "-11",
		name: "kick",
		description: "Kick user",
		options: &[
			user_option("The user to kick"),
			text_option("reason", "The reason for kicking, if any", false),
		],
	},
	BuiltInSpec {
		kind: BuiltIn::Me,
		key: "-5",
		name: "me",
		description: "Displays text with emphasis.",
		options: &[text_option("message", "Your message", true)],
	},
	BuiltInSpec {
		kind: BuiltIn::Msg,
		key: "-14",
		name: "msg",
		description: "Message user",
		options: &[
			user_option("The user to message"),
			text_option("message", "Message to send", true),
		],
	},
	BuiltInSpec {
		kind: BuiltIn::Nick,
		key: "-7",
		name: "nick",
		description: "Change nickname on this server.",
		options: &[limited(
			text_option("new_nick", "New nickname", false),
			NICK_LIMIT,
		)],
	},
	BuiltInSpec {
		kind: BuiltIn::Shrug,
		key: "-1",
		name: "shrug",
		description: "Appends ¯\\_(ツ)_/¯ to your message.",
		options: &[text_option("message", "Your message", false)],
	},
	BuiltInSpec {
		kind: BuiltIn::Spoiler,
		key: "-6",
		name: "spoiler",
		description: "Marks your message as a spoiler.",
		options: &[text_option("message", "Your message", true)],
	},
	BuiltInSpec {
		kind: BuiltIn::Sticker,
		key: "-17",
		name: "sticker",
		description: "Search your stickers",
		options: &[text_option("query", "Search for a sticker", true)],
	},
	BuiltInSpec {
		kind: BuiltIn::Tableflip,
		key: "-2",
		name: "tableflip",
		description: "Appends (╯°□°)╯︵ ┻━┻ to your message.",
		options: &[text_option("message", "Your message", false)],
	},
	BuiltInSpec {
		kind: BuiltIn::Thread,
		key: "-10",
		name: "thread",
		description: "Start new thread",
		options: &[
			limited(
				text_option("name", "Type a name for your thread", true),
				THREAD_NAME_LIMIT,
			),
			text_option("message", "Type the first message in your thread", true),
		],
	},
	BuiltInSpec {
		kind: BuiltIn::Timeout,
		key: "-13",
		name: "timeout",
		description: "Timeout user",
		options: &[
			user_option("The user to timeout"),
			choice_option(
				"duration",
				"How long they should be timed out for",
				&TIMEOUT_DURATIONS,
			),
			text_option("reason", "The reason for timing them out, if any", false),
		],
	},
	BuiltInSpec {
		kind: BuiltIn::Unflip,
		key: "-3",
		name: "unflip",
		description: "Appends ┬─┬ ノ( ゜-゜ノ) to your message.",
		options: &[text_option("message", "Your message", false)],
	},
];

struct Place {
	channel_kind: Option<ChannelType>,
	channel_permissions: Permissions,
}

impl BuiltIn {
	fn allowed(self, access: Option<&GuildAccess>, place: &Place) -> bool {
		let has = |permission| access.is_some_and(|access| access.has(permission));
		let threadable = matches!(
			place.channel_kind,
			Some(ChannelType::GuildText | ChannelType::GuildAnnouncement)
		);
		match self {
			BuiltIn::Ban => has(Permissions::BAN_MEMBERS),
			BuiltIn::Kick => has(Permissions::KICK_MEMBERS),
			BuiltIn::Timeout => has(Permissions::MODERATE_MEMBERS),
			BuiltIn::Nick => has(Permissions::CHANGE_NICKNAME) || has(Permissions::MANAGE_NICKNAMES),
			BuiltIn::Thread => {
				access.is_some()
					&& threadable && place
					.channel_permissions
					.contains(Permissions::CREATE_PUBLIC_THREADS | Permissions::READ_MESSAGE_HISTORY)
			}
			_ => true,
		}
	}

	pub fn message(self, text: &str) -> Option<String> {
		let text = text.trim();
		match self {
			BuiltIn::Shrug => Some(appended(text, "¯\\_(ツ)_/¯")),
			BuiltIn::Tableflip => Some(appended(text, "(╯°□°)╯︵ ┻━┻")),
			BuiltIn::Unflip => Some(appended(text, "┬─┬ノ( º _ ºノ)")),
			BuiltIn::Me if !text.is_empty() => Some(format!("_{text}_")),
			BuiltIn::Spoiler if !text.is_empty() => Some(format!("||{text}||")),
			_ => None,
		}
	}

	pub fn shows_results(self) -> bool {
		matches!(self, BuiltIn::Gif | BuiltIn::Sticker)
	}
}

fn appended(text: &str, face: &str) -> String {
	if text.is_empty() {
		face.to_owned()
	} else {
		format!("{text} {face}")
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandApp {
	pub id: Option<Snowflake>,
	pub name: String,
	pub icon: Option<String>,
	pub hue: usize,
}

impl CommandApp {
	pub fn built_in(&self) -> bool {
		self.id.is_none()
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandSource {
	Application {
		command_id: Snowflake,
		path_kinds: Vec<CommandOptionType>,
	},
	BuiltIn(BuiltIn),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandEntry {
	pub key: String,
	pub source: CommandSource,
	pub app: CommandApp,
	pub path: Vec<String>,
	pub description: String,
	pub options: Vec<CommandOption>,
}

impl CommandEntry {
	pub fn name(&self) -> String {
		self.path.join(" ")
	}

	pub fn required(&self) -> impl Iterator<Item = &CommandOption> {
		self.options.iter().filter(|option| option.is_required())
	}

	pub fn built_in(&self) -> Option<BuiltIn> {
		match self.source {
			CommandSource::BuiltIn(kind) => Some(kind),
			CommandSource::Application { .. } => None,
		}
	}

	pub fn chips(&self) -> impl Iterator<Item = &CommandOption> {
		let listed = self.built_in().is_none_or(|kind| kind == BuiltIn::Gif);
		self.required().filter(move |_| listed)
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSection {
	pub title: String,
	pub app: Option<CommandApp>,
	pub first: usize,
	pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommandPicker {
	pub query: String,
	pub sections: Vec<CommandSection>,
	pub rows: Vec<CommandEntry>,
	pub selected: usize,
}

impl CommandPicker {
	pub fn selected_entry(&self) -> Option<&CommandEntry> {
		self.rows.get(self.selected)
	}

	pub fn selected_section(&self) -> Option<usize> {
		self.sections
			.iter()
			.position(|section| (section.first..section.first + section.count).contains(&self.selected))
	}

	pub fn move_selection(&mut self, delta: i32) {
		let count = self.rows.len();
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

	pub fn jump(&mut self, section: usize) -> bool {
		let Some(first) = self.sections.get(section).map(|section| section.first) else {
			return false;
		};
		self.selected = first;
		true
	}
}

fn application_entries(index: &ApplicationCommandIndex) -> Vec<CommandEntry> {
	let mut entries = Vec::new();
	for (hue, application) in index.applications.iter().enumerate() {
		let app = CommandApp {
			id: Some(application.id),
			name: application.name.clone(),
			icon: application.icon.clone(),
			hue,
		};
		let mut own: Vec<CommandEntry> = index
			.application_commands
			.iter()
			.filter(|command| {
				command.kind == ApplicationCommandType::ChatInput && command.application_id == application.id
			})
			.flat_map(|command| leaves(command, &app))
			.collect();
		own.sort_by_key(CommandEntry::name);
		entries.extend(own);
	}
	entries
}

fn leaves(command: &ApplicationCommand, app: &CommandApp) -> Vec<CommandEntry> {
	let mut found = Vec::new();
	collect_leaves(
		command,
		app,
		vec![command.name.clone()],
		Vec::new(),
		command.description.clone().unwrap_or_default(),
		&command.options,
		&mut found,
	);
	found
}

fn collect_leaves(
	command: &ApplicationCommand,
	app: &CommandApp,
	path: Vec<String>,
	path_kinds: Vec<CommandOptionType>,
	description: String,
	options: &[CommandOption],
	found: &mut Vec<CommandEntry>,
) {
	if !options.iter().any(CommandOption::is_group) {
		let key = match path.get(1..) {
			Some(rest) if !rest.is_empty() => format!("{}\u{0}{}", command.id, rest.join(" ")),
			_ => command.id.to_string(),
		};
		found.push(CommandEntry {
			key,
			source: CommandSource::Application {
				command_id: command.id,
				path_kinds,
			},
			app: app.clone(),
			path,
			description,
			options: options.to_vec(),
		});
		return;
	}
	for group in options.iter().filter(|option| option.is_group()) {
		let mut path = path.clone();
		path.push(group.name.clone());
		let mut kinds = path_kinds.clone();
		kinds.push(group.kind);
		collect_leaves(
			command,
			app,
			path,
			kinds,
			group.description.clone().unwrap_or_default(),
			&group.options,
			found,
		);
	}
}

fn built_in_option(option: &BuiltInOption) -> CommandOption {
	CommandOption {
		kind: option.kind,
		name: option.name.to_owned(),
		description: Some(option.description.to_owned()),
		required: Some(option.required),
		max_length: option.max_length,
		choices: option
			.choices
			.iter()
			.map(|(name, value)| CommandChoice {
				name: (*name).to_owned(),
				value: Value::from(*value),
			})
			.collect::<Vec<_>>()
			.into(),
		..CommandOption::default()
	}
}

fn built_in_entries(access: Option<&GuildAccess>, place: &Place, hue: usize) -> Vec<CommandEntry> {
	let app = CommandApp {
		id: None,
		name: BUILT_IN.to_owned(),
		icon: None,
		hue,
	};
	BUILT_INS
		.iter()
		.filter(|spec| spec.kind.allowed(access, place))
		.map(|spec| CommandEntry {
			key: spec.key.to_owned(),
			source: CommandSource::BuiltIn(spec.kind),
			app: app.clone(),
			path: vec![spec.name.to_owned()],
			description: spec.description.to_owned(),
			options: spec.options.iter().map(built_in_option).collect(),
		})
		.collect()
}

pub fn command_catalog(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
) -> Vec<CommandEntry> {
	let index = state.command_index(channel_id);
	let mut entries = index.map(|index| application_entries(index)).unwrap_or_default();
	let hue = index.map_or(0, |index| index.applications.len());
	let access = guild_id.map(|guild_id| GuildAccess::of(state, guild_id));
	let place = Place {
		channel_kind: state.channel(channel_id).map(|channel| channel.kind),
		channel_permissions: state.my_permissions(channel_id),
	};
	entries.extend(built_in_entries(access.as_ref(), &place, hue));
	entries
}

fn matches(entry: &CommandEntry, query: &str) -> Option<u8> {
	let name = entry.name().to_lowercase();
	if name.starts_with(query) {
		Some(0)
	} else {
		name.contains(query).then_some(1)
	}
}

pub fn command_picker(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	query: &str,
	now_ms: u64,
) -> CommandPicker {
	let catalog = command_catalog(state, guild_id, channel_id);
	let query = query.to_lowercase();
	let mut picker = CommandPicker {
		query: query.clone(),
		..CommandPicker::default()
	};
	if query.is_empty() {
		let frequent: Vec<CommandEntry> = state
			.frequent_commands(now_ms)
			.into_iter()
			.filter_map(|used| catalog.iter().find(|entry| entry.key == used.key))
			.take(FREQUENT_ROWS)
			.cloned()
			.collect();
		push_section(&mut picker, FREQUENTLY_USED.to_owned(), None, frequent);
	}
	let mut groups: Vec<(CommandApp, Vec<(u8, CommandEntry)>)> = Vec::new();
	for entry in catalog {
		let Some(rank) = matches(&entry, &query) else {
			continue;
		};
		match groups.iter_mut().find(|(app, _)| *app == entry.app) {
			Some((_, rows)) => rows.push((rank, entry)),
			None => groups.push((entry.app.clone(), vec![(rank, entry)])),
		}
	}
	for (app, mut rows) in groups {
		rows.sort_by_key(|(rank, _)| *rank);
		let rows = rows.into_iter().map(|(_, entry)| entry).collect();
		push_section(&mut picker, app.name.clone(), Some(app), rows);
	}
	picker
}

fn push_section(picker: &mut CommandPicker, title: String, app: Option<CommandApp>, rows: Vec<CommandEntry>) {
	if rows.is_empty() {
		return;
	}
	picker.sections.push(CommandSection {
		title,
		app,
		first: picker.rows.len(),
		count: rows.len(),
	});
	picker.rows.extend(rows);
}

pub fn exact_command(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	text: &str,
) -> Option<CommandEntry> {
	let name = text.strip_prefix('/')?.strip_suffix(' ')?;
	if name.is_empty() || name.contains(char::is_whitespace) {
		return None;
	}
	let catalog = command_catalog(state, guild_id, channel_id);
	let mut found = catalog
		.into_iter()
		.filter(|entry| entry.path.len() == 1 && entry.path[0].eq_ignore_ascii_case(name));
	let entry = found.next()?;
	found.next().is_none().then_some(entry)
}

fn shown(number: f64) -> String {
	if number.fract() == 0. && number.abs() < 1e15 {
		format!("{}", number as i64)
	} else {
		format!("{number}")
	}
}

fn range_reason(option: &CommandOption) -> String {
	match (option.min_value, option.max_value) {
		(Some(min), Some(max)) => format!("Must be a number between {} and {}", shown(min), shown(max)),
		(Some(min), None) => format!("Must be at least {}", shown(min)),
		(None, Some(max)) => format!("Must be at most {}", shown(max)),
		(None, None) => "Must be a number".to_owned(),
	}
}

fn in_range(option: &CommandOption, number: f64) -> bool {
	option.min_value.is_none_or(|min| number >= min) && option.max_value.is_none_or(|max| number <= max)
}

fn length_reason(option: &CommandOption) -> String {
	match (option.min_length, option.max_length) {
		(Some(min), Some(max)) => format!("Must be between {min} and {max} characters"),
		(Some(min), None) => format!("Must be at least {min} characters"),
		(_, Some(max)) => format!("Must be at most {max} characters"),
		(None, None) => String::new(),
	}
}

fn mention_id(raw: &str, prefixes: &[&str]) -> Option<String> {
	if raw.parse::<u64>().is_ok() {
		return Some(raw.to_owned());
	}
	let inner = raw.strip_prefix('<')?.strip_suffix('>')?;
	prefixes
		.iter()
		.find_map(|prefix| inner.strip_prefix(prefix))
		.filter(|id| id.parse::<u64>().is_ok())
		.map(str::to_owned)
}

fn number_value(option: &CommandOption, raw: &str) -> Result<Value, String> {
	let parsed = match option.kind {
		CommandOptionType::Integer => raw.parse::<i64>().ok().map(|whole| whole as f64),
		_ => raw.parse::<f64>().ok().filter(|number| number.is_finite()),
	};
	let Some(number) = parsed.filter(|number| in_range(option, *number)) else {
		return Err(match (parsed, option.kind) {
			(None, CommandOptionType::Integer)
				if option.min_value.is_none() && option.max_value.is_none() =>
			{
				"Must be a whole number".to_owned()
			}
			_ => range_reason(option),
		});
	};
	match option.kind {
		CommandOptionType::Integer => Ok(Value::from(number as i64)),
		_ => Number::from_f64(number)
			.map(Value::Number)
			.ok_or_else(|| range_reason(option)),
	}
}

fn choice_value(option: &CommandOption, raw: &str) -> Result<Value, String> {
	option
		.choices
		.iter()
		.find(|choice| {
			choice.name.eq_ignore_ascii_case(raw)
				|| match &choice.value {
					Value::String(text) => text.eq_ignore_ascii_case(raw),
					other => serde_json::from_str::<Value>(raw).is_ok_and(|parsed| &parsed == other),
				}
		})
		.map(|choice| choice.value.clone())
		.ok_or_else(|| {
			let names: Vec<&str> = option.choices.iter().map(|choice| choice.name.as_str()).collect();
			format!("Choose one of {}", names.join(", "))
		})
}

pub fn option_value(option: &CommandOption, raw: &str) -> Result<Value, String> {
	let raw = raw.trim();
	if !option.choices.is_empty() {
		return choice_value(option, raw);
	}
	match option.kind {
		CommandOptionType::String => {
			let length = raw.chars().count() as i64;
			let fits = option.min_length.is_none_or(|min| length >= min)
				&& option.max_length.is_none_or(|max| length <= max);
			if fits {
				Ok(Value::String(raw.to_owned()))
			} else {
				Err(length_reason(option))
			}
		}
		CommandOptionType::Integer | CommandOptionType::Number => number_value(option, raw),
		CommandOptionType::Boolean => match raw.to_ascii_lowercase().as_str() {
			"true" | "yes" | "on" => Ok(Value::Bool(true)),
			"false" | "no" | "off" => Ok(Value::Bool(false)),
			_ => Err("Must be True or False".to_owned()),
		},
		CommandOptionType::User => mention_id(raw, &["@!", "@"])
			.map(Value::String)
			.ok_or_else(|| "Must be a user mention or ID".to_owned()),
		CommandOptionType::Channel => mention_id(raw, &["#"])
			.map(Value::String)
			.ok_or_else(|| "Must be a channel mention or ID".to_owned()),
		CommandOptionType::Role => mention_id(raw, &["@&"])
			.map(Value::String)
			.ok_or_else(|| "Must be a role mention or ID".to_owned()),
		CommandOptionType::Mentionable => mention_id(raw, &["@&", "@!", "@"])
			.map(Value::String)
			.ok_or_else(|| "Must be a user or role mention or ID".to_owned()),
		CommandOptionType::Attachment => Err(FILE_REQUIRED.to_owned()),
		_ => Err("This option is not supported".to_owned()),
	}
}

pub fn interaction_options(
	entry: &CommandEntry,
	values: Vec<(CommandOption, Value)>,
) -> Vec<InteractionOption> {
	nested(
		entry,
		values
			.into_iter()
			.map(|(option, value)| InteractionOption {
				kind: option.kind,
				name: option.name,
				value: Some(value),
				options: Vec::new(),
				focused: None,
			})
			.collect(),
	)
}

fn nested(entry: &CommandEntry, mut options: Vec<InteractionOption>) -> Vec<InteractionOption> {
	let CommandSource::Application { path_kinds, .. } = &entry.source else {
		return options;
	};
	for (name, kind) in entry.path.iter().skip(1).zip(path_kinds).rev() {
		options = vec![InteractionOption {
			kind: *kind,
			name: name.clone(),
			value: None,
			options,
			focused: None,
		}];
	}
	options
}

#[derive(Debug, Clone)]
pub struct OptionFile {
	pub id: u64,
	pub file: UploadFile,
}

impl PartialEq for OptionFile {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionField {
	pub option: CommandOption,
	pub value: String,
	pub error: Option<String>,
	pub choice: Option<CommandChoice>,
	pub file: Option<OptionFile>,
}

impl OptionField {
	fn new(option: &CommandOption) -> Self {
		Self {
			option: option.clone(),
			value: String::new(),
			error: None,
			choice: None,
			file: None,
		}
	}

	pub fn takes_file(&self) -> bool {
		self.option.kind == CommandOptionType::Attachment
	}

	pub fn autocompletes(&self) -> bool {
		self.option.autocomplete == Some(true) && !self.takes_file()
	}

	pub fn has_choices(&self) -> bool {
		!self.option.choices.is_empty() || self.autocompletes()
	}

	fn is_empty(&self) -> bool {
		if self.takes_file() {
			self.file.is_none()
		} else {
			self.value.trim().is_empty()
		}
	}

	fn picked(&self) -> Option<Value> {
		self.choice
			.as_ref()
			.filter(|choice| choice.name == self.value)
			.map(|choice| choice.value.clone())
	}

	fn parsed(&self) -> Result<Value, String> {
		match self.picked() {
			Some(value) => Ok(value),
			None => option_value(&self.option, &self.value),
		}
	}

	fn check(&mut self) {
		self.error = if self.value.trim().is_empty() || self.picked().is_some() {
			None
		} else {
			option_value(&self.option, &self.value).err()
		};
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChoiceList {
	Choices(Vec<CommandChoice>),
	Loading,
	Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionChoices {
	pub title: String,
	pub list: ChoiceList,
	pub selected: usize,
}

impl OptionChoices {
	pub fn count(&self) -> usize {
		match &self.list {
			ChoiceList::Choices(choices) => choices.len(),
			_ => 0,
		}
	}

	pub fn selected_choice(&self) -> Option<&CommandChoice> {
		match &self.list {
			ChoiceList::Choices(choices) => choices.get(self.selected),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutocompleteAsk {
	request: u64,
	field: usize,
	value: String,
}

fn filtered(choices: &[CommandChoice], typed: &str) -> Vec<CommandChoice> {
	let typed = typed.trim().to_lowercase();
	choices
		.iter()
		.filter(|choice| typed.is_empty() || choice.name.to_lowercase().contains(&typed))
		.cloned()
		.collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHint {
	pub name: String,
	pub description: String,
	pub error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveCommand {
	pub entry: CommandEntry,
	pub fields: Vec<OptionField>,
	pub focused: usize,
	pub optional_open: bool,
	pub optional_selected: usize,
	pub refusal: Option<String>,
	pub choice_selected: usize,
	asked: Option<AutocompleteAsk>,
}

pub enum Prepared {
	Run(Vec<(CommandOption, Value)>),
	Invalid,
}

impl ActiveCommand {
	pub fn new(entry: CommandEntry) -> Self {
		let fields: Vec<OptionField> = entry.required().map(OptionField::new).collect();
		let mut command = Self {
			entry,
			fields,
			focused: 0,
			optional_open: false,
			optional_selected: 0,
			refusal: None,
			choice_selected: 0,
			asked: None,
		};
		command.optional_open = command.fields.is_empty() && !command.optional().is_empty();
		command
	}

	pub fn optional(&self) -> Vec<&CommandOption> {
		let mut left: Vec<&CommandOption> = self
			.entry
			.options
			.iter()
			.filter(|option| !self.fields.iter().any(|field| field.option.name == option.name))
			.collect();
		left.sort_by_key(|option| !option.is_required());
		left
	}

	pub fn focused_field(&self) -> Option<&OptionField> {
		self.fields.get(self.focused)
	}

	pub fn hint(&self) -> CommandHint {
		let Some(field) = self.focused_field() else {
			return CommandHint {
				name: format!("/{}", self.entry.name()),
				description: self
					.refusal
					.clone()
					.unwrap_or_else(|| self.entry.description.clone()),
				error: self.refusal.is_some(),
			};
		};
		let error = field.error.clone().or_else(|| self.refusal.clone());
		CommandHint {
			name: field.option.name.clone(),
			description: error
				.clone()
				.unwrap_or_else(|| field.option.description.clone().unwrap_or_default()),
			error: error.is_some(),
		}
	}

	pub fn edit(&mut self, index: usize, value: &str) -> bool {
		let Some(field) = self.fields.get_mut(index) else {
			return false;
		};
		if field.value == value {
			return false;
		}
		if field.takes_file() {
			return false;
		}
		field.value = value.to_owned();
		field.check();
		if self.entry.built_in().is_some() && field.option.kind == CommandOptionType::User {
			field.error = None;
		}
		self.refusal = None;
		self.focused = index;
		self.choice_selected = 0;
		true
	}

	pub fn focus(&mut self, index: usize) -> bool {
		if index >= self.fields.len() || index == self.focused {
			return false;
		}
		self.focused = index;
		self.choice_selected = 0;
		true
	}

	pub fn wants_file(&self) -> bool {
		!self.optional_open
			&& self
				.focused_field()
				.is_some_and(|field| field.takes_file() && field.file.is_none())
	}

	pub fn choices(&self, autocomplete: Option<&Autocomplete>) -> Option<OptionChoices> {
		if self.optional_open {
			return None;
		}
		let field = self.focused_field().filter(|field| field.has_choices())?;
		let list = if field.autocompletes() {
			let asked = self.asked.as_ref().filter(|asked| asked.field == self.focused)?;
			match autocomplete.filter(|autocomplete| autocomplete.request == asked.request) {
				Some(Autocomplete {
					status: AutocompleteStatus::Loaded(choices),
					..
				}) => ChoiceList::Choices(choices.clone()),
				Some(Autocomplete {
					status: AutocompleteStatus::Failed,
					..
				}) => ChoiceList::Failed,
				_ => ChoiceList::Loading,
			}
		} else {
			ChoiceList::Choices(filtered(&field.option.choices, &field.value))
		};
		if matches!(&list, ChoiceList::Choices(choices) if choices.is_empty()) {
			return None;
		}
		let last = match &list {
			ChoiceList::Choices(choices) => choices.len() - 1,
			_ => 0,
		};
		Some(OptionChoices {
			title: field.option.name.to_uppercase(),
			list,
			selected: self.choice_selected.min(last),
		})
	}

	pub fn move_choice(&mut self, count: usize, delta: i32) {
		if count == 0 {
			return;
		}
		let current = self.choice_selected.min(count - 1);
		let step = delta.unsigned_abs() as usize % count;
		self.choice_selected = if delta < 0 {
			(current + count - step) % count
		} else {
			(current + step) % count
		};
	}

	pub fn pick_choice(&mut self, choice: CommandChoice) {
		let index = self.focused;
		let Some(field) = self.fields.get_mut(index) else {
			return;
		};
		field.value = choice.name.clone();
		field.choice = Some(choice);
		field.error = None;
		self.refusal = None;
		self.choice_selected = 0;
		if let Some(asked) = &mut self.asked
			&& asked.field == index
		{
			asked.value = field.value.clone();
		}
		self.step(1);
	}

	pub fn attach(&mut self, file: OptionFile) -> bool {
		let Some(field) = self
			.fields
			.get_mut(self.focused)
			.filter(|field| field.takes_file())
		else {
			return false;
		};
		field.file = Some(file);
		field.error = None;
		self.refusal = None;
		self.step(1);
		true
	}

	pub fn clear_file(&mut self, index: usize) -> Option<u64> {
		let removed = self.fields.get_mut(index)?.file.take()?;
		self.focused = index;
		self.optional_open = false;
		Some(removed.id)
	}

	pub fn files(&self) -> Vec<UploadFile> {
		self.fields
			.iter()
			.filter_map(|field| field.file.as_ref().map(|file| file.file.clone()))
			.collect()
	}

	pub fn file_ids(&self) -> Vec<u64> {
		self.fields
			.iter()
			.filter_map(|field| field.file.as_ref().map(|file| file.id))
			.collect()
	}

	fn autocomplete_options(&self) -> Vec<InteractionOption> {
		let leaves = self
			.fields
			.iter()
			.enumerate()
			.filter(|(_, field)| !field.takes_file())
			.filter_map(|(index, field)| {
				let focused = index == self.focused;
				let value = if focused {
					Value::String(field.value.clone())
				} else if field.is_empty() {
					return None;
				} else {
					field.parsed().ok()?
				};
				Some(InteractionOption {
					kind: field.option.kind,
					name: field.option.name.clone(),
					value: Some(value),
					options: Vec::new(),
					focused: focused.then_some(true),
				})
			})
			.collect();
		nested(&self.entry, leaves)
	}

	pub fn autocomplete(&mut self, retry: bool) -> Option<(Snowflake, Vec<InteractionOption>, u64)> {
		let CommandSource::Application { command_id, .. } = &self.entry.source else {
			return None;
		};
		let command_id = *command_id;
		let field = self
			.focused_field()
			.filter(|field| field.autocompletes() && !self.optional_open)?;
		let value = field.value.clone();
		let same = self
			.asked
			.as_ref()
			.is_some_and(|asked| asked.field == self.focused && asked.value == value);
		if same && !retry {
			return None;
		}
		let request = next_autocomplete_request();
		self.asked = Some(AutocompleteAsk {
			request,
			field: self.focused,
			value,
		});
		self.choice_selected = 0;
		Some((command_id, self.autocomplete_options(), request))
	}

	pub fn step(&mut self, delta: i32) -> bool {
		let count = self.fields.len();
		if count == 0 {
			return false;
		}
		let next = if delta < 0 {
			self.focused.checked_sub(1)
		} else {
			Some(self.focused + 1).filter(|next| *next < count)
		};
		next.is_some_and(|next| self.focus(next))
	}

	pub fn toggle_optional(&mut self) -> bool {
		if self.optional().is_empty() {
			return false;
		}
		self.optional_open = !self.optional_open;
		self.optional_selected = 0;
		true
	}

	pub fn move_optional(&mut self, delta: i32) {
		let count = self.optional().len();
		if count == 0 {
			return;
		}
		let step = delta.unsigned_abs() as usize % count;
		self.optional_selected = if delta < 0 {
			(self.optional_selected + count - step) % count
		} else {
			(self.optional_selected + step) % count
		};
	}

	pub fn add_optional(&mut self, index: Option<usize>) -> bool {
		let Some(option) = self
			.optional()
			.get(index.unwrap_or(self.optional_selected))
			.map(|option| (*option).clone())
		else {
			return false;
		};
		self.fields.push(OptionField::new(&option));
		self.focused = self.fields.len() - 1;
		self.optional_open = false;
		self.optional_selected = 0;
		true
	}

	pub fn remove_field(&mut self, index: usize) -> bool {
		let removable = self
			.fields
			.get(index)
			.is_some_and(|field| !field.option.is_required());
		if !removable {
			return false;
		}
		self.fields.remove(index);
		self.focused = index.saturating_sub(1).min(self.fields.len().saturating_sub(1));
		true
	}

	fn fail(&mut self, index: usize, reason: String) -> Prepared {
		if let Some(field) = self.fields.get_mut(index) {
			field.error = Some(reason);
		}
		self.focused = index;
		self.optional_open = false;
		Prepared::Invalid
	}

	pub fn prepare(&mut self) -> Prepared {
		let mut values = Vec::new();
		let mut attachments = 0u64;
		for index in 0..self.fields.len() {
			let field = &self.fields[index];
			if field.is_empty() {
				if field.option.is_required() {
					let reason = if field.takes_file() {
						FILE_REQUIRED
					} else {
						REQUIRED
					};
					return self.fail(index, reason.to_owned());
				}
				continue;
			}
			if field.takes_file() {
				values.push((field.option.clone(), Value::from(attachments)));
				attachments += 1;
				continue;
			}
			match field.parsed() {
				Ok(value) => values.push((field.option.clone(), value)),
				Err(reason) => return self.fail(index, reason),
			}
		}
		Prepared::Run(values)
	}
}
