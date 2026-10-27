use std::rc::Rc;
use std::time::Instant;

use mlua::{Error, Lua, Table, Value};
use protocol::Snowflake;
use session::Command;
use state::State;

use crate::context::Context;
use crate::convert::{to_json, to_lua};
use crate::manifest::{API_VERSION, clip};
use crate::permission::{Action, Permission, Scope};
use crate::requests::PluginRequest;

const LOG_LIMIT: usize = 200;
const ACTIONS_PER_GESTURE: usize = 1;

pub(crate) fn install(lua: &Lua, context: &Rc<Context>) -> mlua::Result<Table> {
	let kyoko = lua.create_table()?;
	kyoko.set("api", API_VERSION)?;
	let plugin = lua.create_table()?;
	plugin.set("id", context.id.as_str())?;
	plugin.set("version", context.version.as_str())?;
	kyoko.set("plugin", plugin)?;
	kyoko.set("log", log_function(lua, context)?)?;
	kyoko.set("settings", settings_table(lua, context)?)?;
	kyoko.set("storage", storage_table(lua, context)?)?;
	kyoko.set("state", state_table(lua, context)?)?;
	kyoko.set("actions", actions_table(lua, context)?)?;
	Ok(kyoko)
}

fn denied(what: &str, permission: &Permission) -> Error {
	Error::runtime(format!("kyoko: {what} needs the {permission} permission"))
}

fn require(context: &Context, what: &str, permission: Permission) -> mlua::Result<()> {
	if context.allows(&permission) {
		Ok(())
	} else {
		Err(denied(what, &permission))
	}
}

fn log_function(lua: &Lua, context: &Rc<Context>) -> mlua::Result<mlua::Function> {
	let context = context.clone();
	lua.create_function(move |_, parts: mlua::Variadic<Value>| {
		let text: Vec<String> = parts
			.iter()
			.map(|part| match part {
				Value::String(text) => text.to_string_lossy(),
				other => format!("{other:?}"),
			})
			.collect();
		tracing::debug!(plugin = %context.id, message = %clip(&text.join(" "), LOG_LIMIT), "plugin log");
		Ok(())
	})
}

fn settings_table(lua: &Lua, context: &Rc<Context>) -> mlua::Result<Table> {
	let table = lua.create_table()?;
	let settings = context.settings.clone();
	table.set(
		"get",
		lua.create_function(move |lua, key: String| {
			let value = settings.read().get(&key).cloned().unwrap_or_default();
			to_lua(lua, &value)
		})?,
	)?;
	let settings = context.settings.clone();
	table.set(
		"all",
		lua.create_function(move |lua, ()| {
			let values = serde_json::Value::Object(settings.read().clone());
			to_lua(lua, &values)
		})?,
	)?;
	Ok(table)
}

fn storage_table(lua: &Lua, context: &Rc<Context>) -> mlua::Result<Table> {
	let table = lua.create_table()?;
	let shared = context.clone();
	table.set(
		"get",
		lua.create_function(move |lua, key: String| {
			require(&shared, "kyoko.storage", Permission::Storage)?;
			let value = shared.storage.borrow().get(&key).cloned().unwrap_or_default();
			to_lua(lua, &value)
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"set",
		lua.create_function(move |_, (key, value): (String, Value)| {
			require(&shared, "kyoko.storage", Permission::Storage)?;
			let json = to_json(&value).map_err(|error| Error::runtime(format!("kyoko.storage: {error}")))?;
			shared
				.storage
				.borrow_mut()
				.set(&key, json)
				.map_err(|error| Error::runtime(format!("kyoko.storage: {error}")))
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"delete",
		lua.create_function(move |_, key: String| {
			require(&shared, "kyoko.storage", Permission::Storage)?;
			shared.storage.borrow_mut().delete(&key);
			Ok(())
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"keys",
		lua.create_function(move |_, ()| {
			require(&shared, "kyoko.storage", Permission::Storage)?;
			Ok(shared.storage.borrow().keys())
		})?,
	)?;
	Ok(table)
}

fn snowflake(text: &str) -> mlua::Result<Snowflake> {
	text.parse::<u64>()
		.map(Snowflake)
		.map_err(|_| Error::runtime("kyoko: ids are strings of digits"))
}

fn with_state<T>(context: &Context, read: impl FnOnce(&State) -> mlua::Result<T>) -> mlua::Result<Option<T>> {
	let Some(attachment) = context.attachment() else {
		return Ok(None);
	};
	let state = attachment.state.read();
	read(&state).map(Some)
}

fn record(lua: &Lua, fields: &[(&str, Value)]) -> mlua::Result<Table> {
	let table = lua.create_table_with_capacity(0, fields.len())?;
	for (key, value) in fields {
		table.raw_set(*key, value.clone())?;
	}
	Ok(table)
}

fn text(lua: &Lua, value: &str) -> mlua::Result<Value> {
	lua.create_string(value).map(Value::String)
}

fn id(lua: &Lua, value: Snowflake) -> mlua::Result<Value> {
	text(lua, &value.to_string())
}

fn optional_id(lua: &Lua, value: Option<Snowflake>) -> mlua::Result<Value> {
	value.map_or(Ok(Value::Nil), |value| id(lua, value))
}

fn user_record(lua: &Lua, user: &protocol::User) -> mlua::Result<Table> {
	record(
		lua,
		&[
			("id", id(lua, user.id)?),
			("username", text(lua, &user.username)?),
			(
				"display_name",
				text(lua, user.global_name.as_deref().unwrap_or(&user.username))?,
			),
			("bot", Value::Boolean(user.bot.unwrap_or(false))),
		],
	)
}

fn channel_record(lua: &Lua, channel: &protocol::Channel) -> mlua::Result<Table> {
	record(
		lua,
		&[
			("id", id(lua, channel.id)?),
			("guild_id", optional_id(lua, channel.guild_id)?),
			("name", text(lua, channel.name.as_deref().unwrap_or_default())?),
			("type", Value::Integer(i64::from(channel.kind))),
			("parent_id", optional_id(lua, channel.parent_id)?),
		],
	)
}

fn state_table(lua: &Lua, context: &Rc<Context>) -> mlua::Result<Table> {
	let table = lua.create_table()?;
	let shared = context.clone();
	table.set(
		"current_user",
		lua.create_function(move |lua, ()| {
			require(
				&shared,
				"kyoko.state.current_user",
				Permission::Read(Scope::Users),
			)?;
			let found = with_state(&shared, |state| {
				state
					.current_user()
					.map(|user| user_record(lua, user))
					.transpose()
			})?;
			Ok(found.flatten())
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"user",
		lua.create_function(move |lua, user_id: String| {
			require(&shared, "kyoko.state.user", Permission::Read(Scope::Users))?;
			let user_id = snowflake(&user_id)?;
			let found = with_state(&shared, |state| {
				state.user(user_id).map(|user| user_record(lua, user)).transpose()
			})?;
			Ok(found.flatten())
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"guilds",
		lua.create_function(move |lua, ()| {
			require(&shared, "kyoko.state.guilds", Permission::Read(Scope::Guilds))?;
			let list = lua.create_table()?;
			with_state(&shared, |state| {
				for guild in state.guilds() {
					list.raw_push(record(
						lua,
						&[("id", id(lua, guild.id)?), ("name", text(lua, &guild.name)?)],
					)?)?;
				}
				Ok(())
			})?;
			Ok(list)
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"channel",
		lua.create_function(move |lua, channel_id: String| {
			let guilds = shared.allows(&Permission::Read(Scope::Guilds));
			let dms = shared.allows(&Permission::Read(Scope::DirectMessages));
			if !guilds && !dms {
				return Err(denied("kyoko.state.channel", &Permission::Read(Scope::Guilds)));
			}
			let channel_id = snowflake(&channel_id)?;
			let found = with_state(&shared, |state| {
				state
					.channel(channel_id)
					.filter(|channel| {
						let in_guild =
							channel.guild_id.is_some() || state.channel_guild(channel.id).is_some();
						if in_guild { guilds } else { dms }
					})
					.map(|channel| channel_record(lua, channel))
					.transpose()
			})?;
			Ok(found.flatten())
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"read_state",
		lua.create_function(move |lua, channel_id: String| {
			require(
				&shared,
				"kyoko.state.read_state",
				Permission::Read(Scope::ReadState),
			)?;
			let channel_id = snowflake(&channel_id)?;
			let found = with_state(&shared, |state| {
				state
					.read_state(channel_id)
					.map(|read| {
						record(
							lua,
							&[
								("channel_id", id(lua, channel_id)?),
								("last_message_id", optional_id(lua, read.last_message_id)?),
								("mention_count", Value::Integer(read.mention_count.unwrap_or(0))),
							],
						)
					})
					.transpose()
			})?;
			Ok(found.flatten())
		})?,
	)?;
	Ok(table)
}

fn permit(context: &Context, what: &str, action: Action) -> mlua::Result<crate::context::Attachment> {
	require(context, what, Permission::Action(action))?;
	if !context.gesture.get() {
		return Err(Error::runtime(format!(
			"kyoko: {what} was refused: actions only run from your click or command"
		)));
	}
	if context.gesture_actions.get() >= ACTIONS_PER_GESTURE {
		return Err(Error::runtime(format!(
			"kyoko: {what} was refused: one action per click"
		)));
	}
	let attachment = context
		.attachment()
		.ok_or_else(|| Error::runtime(format!("kyoko: {what} was refused: not signed in")))?;
	let now = Instant::now();
	let mut log = context.actions.lock();
	while log
		.front()
		.is_some_and(|at| now.duration_since(*at) >= context.limits.action_window)
	{
		log.pop_front();
	}
	if log.len() >= context.limits.actions {
		return Err(Error::runtime(format!(
			"kyoko: {what} was refused: too many actions, try again shortly"
		)));
	}
	log.push_back(now);
	context.gesture_actions.set(context.gesture_actions.get() + 1);
	tracing::info!(plugin = %context.id, action = what, "plugin action from a user gesture");
	Ok(attachment)
}

fn act(context: &Context, what: &str, action: Action, command: Command) -> mlua::Result<()> {
	permit(context, what, action)?.commander.send(command);
	Ok(())
}

fn clone_expression(context: &Context) -> mlua::Result<()> {
	let what = "kyoko.actions.clone_expression";
	let target = context
		.gesture_target
		.borrow()
		.clone()
		.filter(state::Expression::is_image)
		.ok_or_else(|| {
			Error::runtime(format!(
				"kyoko: {what} was refused: it clones only the custom emoji or image sticker you chose from a menu"
			))
		})?;
	permit(context, what, Action::UploadExpression)?;
	(context.requests)(PluginRequest::Clone {
		plugin: context.id.clone(),
		expression: target,
	});
	Ok(())
}

fn share_activity(context: &Context, wanted: Option<bool>) -> mlua::Result<bool> {
	let attachment = permit(context, "kyoko.actions.share_activity", Action::Activity)?;
	let shared = wanted.unwrap_or_else(|| !attachment.state.read().status_settings().shares_activity());
	attachment.commander.send(Command::SetActivitySharing(shared));
	(context.requests)(PluginRequest::ActivitySharing {
		plugin: context.id.clone(),
		shared,
	});
	Ok(shared)
}

fn actions_table(lua: &Lua, context: &Rc<Context>) -> mlua::Result<Table> {
	let table = lua.create_table()?;
	let shared = context.clone();
	table.set(
		"mark_all_read",
		lua.create_function(move |_, ()| {
			act(
				&shared,
				"kyoko.actions.mark_all_read",
				Action::MarkRead,
				Command::MarkAllRead,
			)
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"mark_guild_read",
		lua.create_function(move |_, guild_id: String| {
			let guild_id = snowflake(&guild_id)?;
			act(
				&shared,
				"kyoko.actions.mark_guild_read",
				Action::MarkRead,
				Command::MarkGuildRead { guild_id },
			)
		})?,
	)?;
	let shared = context.clone();
	table.set(
		"clone_expression",
		lua.create_function(move |_, ()| clone_expression(&shared))?,
	)?;
	let shared = context.clone();
	table.set(
		"share_activity",
		lua.create_function(move |_, wanted: Option<bool>| share_activity(&shared, wanted))?,
	)?;
	Ok(table)
}
