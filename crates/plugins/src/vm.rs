use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::chunk::ChunkMode;
use mlua::{Error, Function, Lua, RegistryKey, Table, Value, VmState};
use session::{DeletedLook, HistoryLook, KeptMessage, ShownName};

use crate::api;
use crate::calls::{
	Call, Fault, Look, MessageView, ON_EVENT, PluginEvent, RANK_EMOJI, RENDER_MESSAGE, Reply, SHOW_DELETED,
};
use crate::context::Context;
use crate::manifest::clip;
use crate::package::Package;

const ERROR_LIMIT: usize = 160;
const TEXT_LIMIT: usize = 4000;
const LABEL_LIMIT: usize = 100;
const ORDER_LIMIT: usize = 512;
const ACTION_KEY_LIMIT: usize = 48;
const HOOK_NAMES: [&str; 12] = [
	"before_send",
	"before_edit",
	"allow_typing",
	"allow_notification",
	"format_name",
	"format_relative",
	RANK_EMOJI,
	"on_action",
	"on_settings_changed",
	ON_EVENT,
	RENDER_MESSAGE,
	SHOW_DELETED,
];

pub(crate) struct LogicVm {
	lua: Lua,
	hooks: RegistryKey,
	context: Rc<Context>,
	defined: BTreeSet<String>,
}

impl LogicVm {
	pub(crate) fn load(package: &Package, context: Rc<Context>) -> Result<LogicVm, String> {
		let lua = Lua::new();
		lua.set_memory_limit(context.limits.memory).map_err(describe)?;
		let watched = context.clone();
		lua.set_interrupt(move |_| {
			if let Some(deadline) = watched.deadline.get()
				&& Instant::now() >= deadline
			{
				watched.overran.set(true);
				return Err(Error::runtime("the plugin ran past its time limit"));
			}
			Ok(VmState::Continue)
		});
		let globals = lua.globals();
		globals
			.set("kyoko", api::install(&lua, &context).map_err(describe)?)
			.map_err(describe)?;
		globals
			.set(
				"require",
				require_function(&lua, package.clone()).map_err(describe)?,
			)
			.map_err(describe)?;
		globals.set("print", Value::Nil).map_err(describe)?;
		lua.sandbox(true).map_err(describe)?;
		let (name, source) = package
			.logic_source()
			.ok_or_else(|| "the plugin has no logic.luau".to_owned())?;
		context.overran.set(false);
		context
			.deadline
			.set(Some(Instant::now() + context.limits.load_deadline));
		let loaded = lua
			.load(source)
			.set_name(format!("@{}/{name}", package.id()))
			.set_mode(ChunkMode::Text)
			.call::<Value>(());
		context.deadline.set(None);
		let hooks = match loaded {
			Ok(Value::Table(table)) => table,
			Ok(_) => return Err(format!("{name} must return a table of hooks")),
			Err(_) if context.overran.get() => return Err(format!("{name} took too long to load")),
			Err(error) => return Err(describe(error)),
		};
		let defined = HOOK_NAMES
			.iter()
			.filter(|hook| matches!(hooks.raw_get::<Value>(**hook), Ok(Value::Function(_))))
			.map(|hook| (*hook).to_owned())
			.collect();
		let hooks = lua.create_registry_value(hooks).map_err(describe)?;
		Ok(LogicVm {
			lua,
			hooks,
			context,
			defined,
		})
	}

	pub(crate) fn defined(&self) -> &BTreeSet<String> {
		&self.defined
	}

	pub(crate) fn run(&self, call: &Call, deadline: Duration) -> Result<Reply, Fault> {
		let hook = call.hook_name();
		let Some(function) = self.function(hook) else {
			return Ok(default_reply(call));
		};
		let arguments = self
			.arguments(call)
			.map_err(|error| Fault::Error(describe(error)))?;
		self.context.gesture.set(call.is_gesture());
		self.context.gesture_actions.set(0);
		*self.context.gesture_target.borrow_mut() = call.target();
		let result = self.guarded(deadline, || function.call::<Value>(arguments));
		self.context.gesture.set(false);
		self.context.gesture_target.borrow_mut().take();
		self.context.storage.borrow_mut().flush();
		let value = result?;
		Ok(match call {
			Call::BeforeSend { content, .. } | Call::BeforeEdit { content, .. } => {
				Reply::Text(rewrite(&value, content))
			}
			Call::AllowTyping { .. } | Call::AllowNotification(_) => {
				Reply::Allow(!matches!(value, Value::Boolean(false)))
			}
			Call::FormatName { shown, .. } => Reply::Name(shown_name(&value, shown)),
			Call::FormatRelative(time) => Reply::Text(label(&value).filter(|text| *text != time.text)),
			Call::RankEmoji { .. } => Reply::Order(order(&value)),
			Call::RenderMessage(_) => Reply::Look(look(&value)),
			Call::ShowDeleted => Reply::Allow(matches!(value, Value::Boolean(true))),
			Call::Action { .. } | Call::SettingsChanged => Reply::Done,
		})
	}

	pub(crate) fn deliver(&self, event: &PluginEvent, deadline: Duration) -> Result<(), Fault> {
		let Some(function) = self.function(ON_EVENT) else {
			return Ok(());
		};
		let argument = self
			.event_table(event)
			.map_err(|error| Fault::Error(describe(error)))?;
		let result = self.guarded(deadline, || function.call::<()>(argument));
		self.context.storage.borrow_mut().flush();
		result
	}

	fn function(&self, hook: &str) -> Option<Function> {
		let hooks: Table = self.lua.registry_value(&self.hooks).ok()?;
		match hooks.raw_get::<Value>(hook) {
			Ok(Value::Function(function)) => Some(function),
			_ => None,
		}
	}

	fn guarded<T>(&self, deadline: Duration, run: impl FnOnce() -> mlua::Result<T>) -> Result<T, Fault> {
		self.context.overran.set(false);
		self.context.deadline.set(Some(Instant::now() + deadline));
		let result = run();
		self.context.deadline.set(None);
		match result {
			Ok(value) => Ok(value),
			Err(_) if self.context.overran.get() => Err(Fault::Overrun),
			Err(error) => Err(Fault::Error(describe(error))),
		}
	}

	fn arguments(&self, call: &Call) -> mlua::Result<mlua::MultiValue> {
		let lua = &self.lua;
		let values: Vec<Value> = match call {
			Call::BeforeSend {
				channel_id,
				guild_id,
				content,
			} => vec![Value::Table(table(
				lua,
				&[
					("channel_id", Some(channel_id.to_string())),
					("guild_id", guild_id.map(|id| id.to_string())),
					("content", Some(content.clone())),
				],
			)?)],
			Call::BeforeEdit {
				channel_id,
				guild_id,
				message_id,
				content,
			} => vec![Value::Table(table(
				lua,
				&[
					("channel_id", Some(channel_id.to_string())),
					("guild_id", guild_id.map(|id| id.to_string())),
					("message_id", Some(message_id.to_string())),
					("content", Some(content.clone())),
				],
			)?)],
			Call::AllowTyping { channel_id, guild_id } => vec![Value::Table(table(
				lua,
				&[
					("channel_id", Some(channel_id.to_string())),
					("guild_id", guild_id.map(|id| id.to_string())),
				],
			)?)],
			Call::AllowNotification(message) => vec![Value::Table(message_table(lua, message)?)],
			Call::FormatName { person, shown } => {
				let argument = table(
					lua,
					&[
						("user_id", Some(person.user_id.to_string())),
						("guild_id", person.guild_id.map(|id| id.to_string())),
						("username", Some(person.username.clone())),
						("global_name", person.global_name.clone()),
						("nick", person.nick.clone()),
						("name", Some(shown.name.clone())),
						("extra", shown.extra.clone()),
					],
				)?;
				vec![Value::Table(argument)]
			}
			Call::FormatRelative(time) => {
				let argument = table(
					lua,
					&[
						("unit", Some(time.unit.to_owned())),
						("text", Some(time.text.clone())),
					],
				)?;
				argument.raw_set("seconds", time.seconds.min(i64::MAX as u64) as i64)?;
				argument.raw_set("past", time.past)?;
				argument.raw_set("value", time.value)?;
				vec![Value::Table(argument)]
			}
			Call::RankEmoji {
				guild_id,
				query,
				emojis,
			} => {
				let request = table(
					lua,
					&[
						("query", Some(query.clone())),
						("guild_id", guild_id.map(|id| id.to_string())),
					],
				)?;
				let list = lua.create_table_with_capacity(emojis.len(), 0)?;
				for ranked in emojis {
					list.raw_push(emoji_table(lua, ranked)?)?;
				}
				request.raw_set("emojis", list)?;
				vec![Value::Table(request)]
			}
			Call::Action { key, context, .. } => {
				let details = lua.create_table()?;
				for (name, value) in context {
					details.raw_set(name.as_str(), value.as_str())?;
				}
				vec![Value::String(lua.create_string(key)?), Value::Table(details)]
			}
			Call::RenderMessage(message) => vec![Value::Table(kept_table(lua, message)?)],
			Call::SettingsChanged | Call::ShowDeleted => Vec::new(),
		};
		Ok(mlua::MultiValue::from_iter(values))
	}

	fn event_table(&self, event: &PluginEvent) -> mlua::Result<Table> {
		let lua = &self.lua;
		let fields: Vec<(&str, Option<String>)> = match event {
			PluginEvent::Ready { user_id } => vec![("user_id", Some(user_id.to_string()))],
			PluginEvent::MessageCreate(message) => {
				let event = lua.create_table()?;
				event.raw_set("type", "message_create")?;
				event.raw_set("channel_id", message.channel_id.to_string())?;
				event.raw_set("guild_id", message.guild_id.map(|id| id.to_string()))?;
				event.raw_set("message", message_table(lua, message)?)?;
				return Ok(event);
			}
			PluginEvent::MessageUpdate {
				id,
				channel_id,
				guild_id,
				content,
			} => vec![
				("id", Some(id.to_string())),
				("channel_id", Some(channel_id.to_string())),
				("guild_id", guild_id.map(|id| id.to_string())),
				("content", content.clone()),
			],
			PluginEvent::MessageDelete {
				id,
				channel_id,
				guild_id,
			} => vec![
				("id", Some(id.to_string())),
				("channel_id", Some(channel_id.to_string())),
				("guild_id", guild_id.map(|id| id.to_string())),
			],
			PluginEvent::TypingStart {
				channel_id,
				guild_id,
				user_id,
			} => vec![
				("channel_id", Some(channel_id.to_string())),
				("guild_id", guild_id.map(|id| id.to_string())),
				("user_id", Some(user_id.to_string())),
			],
			PluginEvent::PresenceUpdate {
				user_id,
				guild_id,
				status,
			} => vec![
				("user_id", Some(user_id.to_string())),
				("guild_id", guild_id.map(|id| id.to_string())),
				("status", Some(status.clone())),
			],
			PluginEvent::MessageAck {
				channel_id,
				message_id,
			} => vec![
				("channel_id", Some(channel_id.to_string())),
				("message_id", Some(message_id.to_string())),
			],
			PluginEvent::GuildCreate { guild_id, name } => {
				vec![
					("guild_id", Some(guild_id.to_string())),
					("name", Some(name.clone())),
				]
			}
			PluginEvent::GuildDelete { guild_id } => vec![("guild_id", Some(guild_id.to_string()))],
		};
		let event_table = table(lua, &fields)?;
		event_table.raw_set("type", event.name())?;
		Ok(event_table)
	}
}

fn table(lua: &Lua, fields: &[(&str, Option<String>)]) -> mlua::Result<Table> {
	let table = lua.create_table_with_capacity(0, fields.len())?;
	for (key, value) in fields {
		if let Some(value) = value {
			table.raw_set(*key, value.as_str())?;
		}
	}
	Ok(table)
}

fn message_table(lua: &Lua, message: &MessageView) -> mlua::Result<Table> {
	table(
		lua,
		&[
			("id", Some(message.id.to_string())),
			("channel_id", Some(message.channel_id.to_string())),
			("guild_id", message.guild_id.map(|id| id.to_string())),
			("author_id", Some(message.author_id.to_string())),
			("author_name", Some(message.author_name.clone())),
			("content", Some(message.content.clone())),
		],
	)
}

fn emoji_table(lua: &Lua, ranked: &session::RankedEmoji) -> mlua::Result<Table> {
	let emoji = &ranked.emoji;
	let entry = table(
		lua,
		&[
			("key", Some(emoji.key.clone())),
			("name", Some(emoji.name.clone())),
			("id", emoji.id.map(|id| id.to_string())),
			("guild_id", emoji.guild_id.map(|id| id.to_string())),
			("emoji", emoji.unicode.clone()),
		],
	)?;
	entry.raw_set("animated", emoji.animated)?;
	if let Some(position) = ranked.favorite {
		entry.raw_set("favorite", position + 1)?;
	}
	Ok(entry)
}

fn order(value: &Value) -> Option<Vec<String>> {
	let Value::Table(keys) = value else {
		return None;
	};
	let order = keys
		.sequence_values::<Value>()
		.take(ORDER_LIMIT)
		.filter_map(|key| match key {
			Ok(Value::String(key)) => key.to_str().ok().map(|key| key.to_owned()),
			_ => None,
		})
		.collect();
	Some(order)
}

fn default_reply(call: &Call) -> Reply {
	match call {
		Call::BeforeSend { .. } | Call::BeforeEdit { .. } => Reply::Text(None),
		Call::AllowTyping { .. } | Call::AllowNotification(_) => Reply::Allow(true),
		Call::FormatName { .. } => Reply::Name(None),
		Call::FormatRelative(_) => Reply::Text(None),
		Call::RankEmoji { .. } => Reply::Order(None),
		Call::RenderMessage(_) => Reply::Look(None),
		Call::ShowDeleted => Reply::Allow(false),
		Call::Action { .. } | Call::SettingsChanged => Reply::Done,
	}
}

fn kept_table(lua: &Lua, message: &KeptMessage) -> mlua::Result<Table> {
	let argument = table(
		lua,
		&[
			("id", Some(message.id.to_string())),
			("channel_id", Some(message.channel_id.to_string())),
			("guild_id", message.guild_id.map(|id| id.to_string())),
			("author_id", Some(message.author_id.to_string())),
			("content", Some(message.content.clone())),
		],
	)?;
	argument.raw_set("bot", message.bot)?;
	argument.raw_set("mine", message.mine)?;
	argument.raw_set("deleted", message.deleted_at.is_some())?;
	argument.raw_set("deleted_at", message.deleted_at)?;
	argument.raw_set("edited_at", message.edited_at)?;
	let versions = lua.create_table_with_capacity(message.versions.len(), 0)?;
	for version in &message.versions {
		let entry = table(lua, &[("content", Some(version.content.clone()))])?;
		entry.raw_set("edited_at", version.edited_at)?;
		entry.raw_set("replaced_at", version.replaced_at)?;
		entry.raw_set("attachments", version.attachments)?;
		entry.raw_set("embeds", version.embeds)?;
		versions.raw_push(entry)?;
	}
	argument.raw_set("versions", versions)?;
	Ok(argument)
}

fn word(table: &Table, key: &str) -> Option<String> {
	match table.raw_get::<Value>(key).ok()? {
		Value::String(text) => text.to_str().ok().map(|text| text.to_owned()),
		_ => None,
	}
}

fn look(value: &Value) -> Option<Look> {
	let Value::Table(table) = value else {
		return None;
	};
	let deleted = match word(table, "deleted").as_deref() {
		Some("text") => Some(DeletedLook::Text),
		Some("tint") => Some(DeletedLook::Tint),
		Some("hide") => Some(DeletedLook::Hidden),
		_ => None,
	};
	let history = match word(table, "history").as_deref() {
		Some("inline") => Some(HistoryLook::Inline),
		Some("collapsed") => Some(HistoryLook::Collapsed),
		_ => None,
	};
	let accessory = match table.raw_get::<Value>("accessory").ok()? {
		text @ Value::String(_) => label(&text).map(|text| (text, None)),
		Value::Table(accessory) => accessory
			.raw_get::<Value>("text")
			.ok()
			.and_then(|text| label(&text))
			.map(|text| {
				let action =
					word(&accessory, "action").filter(|key| !key.is_empty() && key.len() <= ACTION_KEY_LIMIT);
				(text, action)
			}),
		_ => None,
	};
	let look = Look {
		deleted,
		history,
		accessory,
	};
	(look != Look::default()).then_some(look)
}

fn label(value: &Value) -> Option<String> {
	let Value::String(text) = value else {
		return None;
	};
	let text = text.to_str().ok()?.to_owned();
	(!text.trim().is_empty() && text.chars().count() <= LABEL_LIMIT).then_some(text)
}

fn shown_name(value: &Value, current: &ShownName) -> Option<ShownName> {
	let shown = match value {
		Value::String(_) => ShownName {
			name: label(value)?,
			extra: None,
		},
		Value::Table(table) => {
			let name = match table.raw_get::<Value>("name").ok()? {
				Value::Nil => current.name.clone(),
				name => label(&name)?,
			};
			let extra = table
				.raw_get::<Value>("extra")
				.ok()
				.and_then(|extra| label(&extra));
			ShownName { name, extra }
		}
		_ => return None,
	};
	(shown != *current).then_some(shown)
}

fn rewrite(value: &Value, original: &str) -> Option<String> {
	let Value::String(text) = value else {
		return None;
	};
	let text = text.to_str().ok()?.to_owned();
	let usable = !text.trim().is_empty() && text.chars().count() <= TEXT_LIMIT && text != original;
	usable.then_some(text)
}

fn require_function(lua: &Lua, package: Package) -> mlua::Result<Function> {
	let loaded: Rc<RefCell<HashMap<String, RegistryKey>>> = Rc::new(RefCell::new(HashMap::new()));
	lua.create_function(move |lua, name: String| {
		let file = module_file(&name)
			.ok_or_else(|| Error::runtime(format!("require: {name:?} is not a module of this plugin")))?;
		if let Some(key) = loaded.borrow().get(&file) {
			return lua.registry_value::<Value>(key);
		}
		let source = package
			.read(&file)
			.ok_or_else(|| Error::runtime(format!("require: {file} is not in this plugin")))?;
		let value = lua
			.load(source)
			.set_name(format!("@{}/{file}", package.id()))
			.set_mode(ChunkMode::Text)
			.call::<Value>(())?;
		let key = lua.create_registry_value(value.clone())?;
		loaded.borrow_mut().insert(file, key);
		Ok(value)
	})
}

fn module_file(name: &str) -> Option<String> {
	let bare = name.strip_prefix("./").unwrap_or(name);
	let file = if bare.ends_with(".luau") {
		bare.to_owned()
	} else {
		format!("{bare}.luau")
	};
	crate::package::is_module_file(&file).then_some(file)
}

fn describe(error: Error) -> String {
	let text = match &error {
		Error::MemoryError(_) => "the plugin ran out of memory".to_owned(),
		Error::CallbackError { cause, .. } => cause.to_string(),
		other => other.to_string(),
	};
	clip(text.lines().next().unwrap_or_default(), ERROR_LIMIT)
}
