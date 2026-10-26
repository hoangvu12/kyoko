use protocol::{
	ApplicationCommand, ApplicationCommandIndex, ApplicationCommandType, CommandOption, CommandOptionType,
	InteractionOption, Permissions, Snowflake,
};
use serde_json::{Number, Value};
use state::State;

use crate::guild_admin::GuildAccess;

const FREQUENT_ROWS: usize = 5;
const FREQUENTLY_USED: &str = "Frequently Used";
const BUILT_IN: &str = "Built-In";
const REQUIRED: &str = "This option is required";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltIn {
	Ban,
	Gif,
	Kick,
	Me,
	Msg,
	Nick,
	Schedule,
	Shrug,
	Spoiler,
	Sticker,
	Tableflip,
	Thread,
	Timeout,
	Unflip,
}

struct BuiltInSpec {
	kind: BuiltIn,
	key: &'static str,
	name: &'static str,
	description: &'static str,
	option: Option<(&'static str, &'static str, bool)>,
}

const BUILT_INS: [BuiltInSpec; 14] = [
	BuiltInSpec {
		kind: BuiltIn::Ban,
		key: "-12",
		name: "ban",
		description: "Ban user",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Gif,
		key: "-4",
		name: "gif",
		description: "Search Animated GIFs on the Web",
		option: Some(("query", "What to search for", true)),
	},
	BuiltInSpec {
		kind: BuiltIn::Kick,
		key: "-11",
		name: "kick",
		description: "Kick user",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Me,
		key: "-6",
		name: "me",
		description: "Displays text with emphasis.",
		option: Some(("message", "The text to emphasise", false)),
	},
	BuiltInSpec {
		kind: BuiltIn::Msg,
		key: "-15",
		name: "msg",
		description: "Message user",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Nick,
		key: "-8",
		name: "nick",
		description: "Change nickname on this server.",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Schedule,
		key: "-16",
		name: "schedule",
		description: "Schedule a message to send later",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Shrug,
		key: "-1",
		name: "shrug",
		description: "Appends ¯\\_(ツ)_/¯ to your message.",
		option: Some(("message", "Your message", false)),
	},
	BuiltInSpec {
		kind: BuiltIn::Spoiler,
		key: "-7",
		name: "spoiler",
		description: "Marks your message as a spoiler.",
		option: Some(("message", "The text to hide", false)),
	},
	BuiltInSpec {
		kind: BuiltIn::Sticker,
		key: "-14",
		name: "sticker",
		description: "Search your stickers",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Tableflip,
		key: "-2",
		name: "tableflip",
		description: "Appends (╯°□°)╯︵ ┻━┻ to your message.",
		option: Some(("message", "Your message", false)),
	},
	BuiltInSpec {
		kind: BuiltIn::Thread,
		key: "-10",
		name: "thread",
		description: "Start new thread",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Timeout,
		key: "-13",
		name: "timeout",
		description: "Timeout user",
		option: None,
	},
	BuiltInSpec {
		kind: BuiltIn::Unflip,
		key: "-3",
		name: "unflip",
		description: "Appends ┬─┬ノ( º _ ºノ) to your message.",
		option: Some(("message", "Your message", false)),
	},
];

impl BuiltIn {
	fn allowed(self, access: Option<&GuildAccess>) -> bool {
		match self {
			BuiltIn::Ban => access.is_some_and(|access| access.has(Permissions::BAN_MEMBERS)),
			BuiltIn::Kick => access.is_some_and(|access| access.has(Permissions::KICK_MEMBERS)),
			BuiltIn::Timeout => access.is_some_and(|access| access.has(Permissions::MODERATE_MEMBERS)),
			BuiltIn::Nick | BuiltIn::Thread => access.is_some(),
			_ => true,
		}
	}

	pub fn message(self, text: &str) -> Option<String> {
		let text = text.trim();
		match self {
			BuiltIn::Shrug => Some(appended(text, "¯\\\\\\_(ツ)\\_/¯")),
			BuiltIn::Tableflip => Some(appended(text, "(╯°□°)╯︵ ┻━┻")),
			BuiltIn::Unflip => Some(appended(text, "┬─┬ノ( º _ ºノ)")),
			BuiltIn::Me if !text.is_empty() => Some(format!("_{text}_")),
			BuiltIn::Spoiler if !text.is_empty() => Some(format!("||{text}||")),
			_ => None,
		}
	}

	fn needs_text(self) -> bool {
		matches!(self, BuiltIn::Me | BuiltIn::Spoiler)
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

fn built_in_option(name: &str, description: &str, required: bool) -> CommandOption {
	CommandOption {
		kind: CommandOptionType::String,
		name: name.to_owned(),
		description: Some(description.to_owned()),
		required: Some(required),
		..CommandOption::default()
	}
}

fn built_in_entries(access: Option<&GuildAccess>, hue: usize) -> Vec<CommandEntry> {
	let app = CommandApp {
		id: None,
		name: BUILT_IN.to_owned(),
		icon: None,
		hue,
	};
	BUILT_INS
		.iter()
		.filter(|spec| spec.kind.allowed(access))
		.map(|spec| CommandEntry {
			key: spec.key.to_owned(),
			source: CommandSource::BuiltIn(spec.kind),
			app: app.clone(),
			path: vec![spec.name.to_owned()],
			description: spec.description.to_owned(),
			options: spec
				.option
				.map(|(name, description, required)| built_in_option(name, description, required))
				.into_iter()
				.collect(),
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
	entries.extend(built_in_entries(access.as_ref(), hue));
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
		CommandOptionType::Attachment => Err("File options are not supported yet".to_owned()),
		_ => Err("This option is not supported".to_owned()),
	}
}

pub fn interaction_options(
	entry: &CommandEntry,
	values: Vec<(CommandOption, Value)>,
) -> Vec<InteractionOption> {
	let mut options: Vec<InteractionOption> = values
		.into_iter()
		.map(|(option, value)| InteractionOption {
			kind: option.kind,
			name: option.name,
			value: Some(value),
			options: Vec::new(),
		})
		.collect();
	let CommandSource::Application { path_kinds, .. } = &entry.source else {
		return options;
	};
	for (name, kind) in entry.path.iter().skip(1).zip(path_kinds).rev() {
		options = vec![InteractionOption {
			kind: *kind,
			name: name.clone(),
			value: None,
			options,
		}];
	}
	options
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionField {
	pub option: CommandOption,
	pub value: String,
	pub error: Option<String>,
}

impl OptionField {
	fn new(option: &CommandOption) -> Self {
		Self {
			option: option.clone(),
			value: String::new(),
			error: None,
		}
	}

	fn check(&mut self) {
		self.error = if self.value.trim().is_empty() {
			None
		} else {
			option_value(&self.option, &self.value).err()
		};
	}
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
}

pub enum Prepared {
	Run(Vec<(CommandOption, Value)>),
	Invalid,
}

impl ActiveCommand {
	pub fn new(entry: CommandEntry) -> Self {
		let fields = entry.required().map(OptionField::new).collect();
		Self {
			entry,
			fields,
			focused: 0,
			optional_open: false,
			optional_selected: 0,
			refusal: None,
		}
	}

	pub fn optional(&self) -> Vec<&CommandOption> {
		self.entry
			.options
			.iter()
			.filter(|option| !option.is_required())
			.filter(|option| !self.fields.iter().any(|field| field.option.name == option.name))
			.collect()
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
		field.value = value.to_owned();
		field.check();
		self.refusal = None;
		self.focused = index;
		true
	}

	pub fn focus(&mut self, index: usize) -> bool {
		if index >= self.fields.len() || index == self.focused {
			return false;
		}
		self.focused = index;
		true
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
		for index in 0..self.fields.len() {
			let field = &self.fields[index];
			if field.value.trim().is_empty() {
				if field.option.is_required() {
					return self.fail(index, REQUIRED.to_owned());
				}
				continue;
			}
			match option_value(&field.option, &field.value) {
				Ok(value) => values.push((field.option.clone(), value)),
				Err(reason) => return self.fail(index, reason),
			}
		}
		if let CommandSource::BuiltIn(kind) = self.entry.source
			&& kind.needs_text()
			&& values.is_empty()
		{
			let index = self
				.fields
				.iter()
				.position(|field| field.option.name == "message");
			return match index {
				Some(index) => self.fail(index, REQUIRED.to_owned()),
				None => {
					self.refusal = Some("Add a message first".to_owned());
					Prepared::Invalid
				}
			};
		}
		Prepared::Run(values)
	}
}
